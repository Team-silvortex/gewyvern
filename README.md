# gewyvern v0.5.6

Protocol-agnostic network debugging runtime driven by eBPF fragments.

`gewyvern` is not trying to be a long-running observability platform. The
current shape is a single-host, window-bounded debugger/runtime that:

- composes eBPF fragments into an attach plan
- ingests structured kernel facts
- reconstructs transport flows and higher-level program flows
- derives deterministic reason chains
- exports a replayable JSON bundle

The long-term direction is:

- fragments are the embryo of runtime IR
- templates compose fragments plus runtime policies
- DSL should compile into template bindings plus fragment parameters
- protocol behavior should eventually be driven by a DSL over that IR
- all protocol behavior should stay grounded in existing eBPF fragment templates,
  not ad hoc generated kernel bytecode

## Start Here

If you want the shortest path into the current system:

```bash
# Discover built-in protocol coverage
cargo run -- --list-protocols
cargo run -- --list-entries quic

# Run one built-in protocol path
cargo run -- --protocol mysql --entry session --json --summary-only
cargo run -- --protocol mysql --entry session --report-format html --out /tmp/mysql-session-report.html

# Sweep the default built-in protocol set
cargo run -- --scan-all --json --summary-only

# Render the same sweep as a visual HTML report
cargo run -- --scan-all --summary-only --report-format html --out /tmp/gewyvern-scan-report.html

# Compile a DSL file or package without starting the runtime
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy --json
```

`--summary-only --json` is now the fastest operational view: it includes a
`protocol_flows` array and `process_network_profiles` summary that show whether
each matched protocol path is healthy or currently stuck at a missing
transition. `--report-format html` renders the same single-target or full scan
as a visual report.

## Status

- project version: `0.5.6`
- stage: working prototype with a stabilized workspace, protocol registry, and package-driven DSL/compiler path
- transport support: TCP + UDP
- protocol path coverage in DSL: DNS, HTTP, TLS, QUIC, STUN, CoAP, NTP, DHCP, WireGuard, mDNS, SSDP, Redis, MQTT, PostgreSQL, MySQL, Memcached, AMQP, RADIUS, GTP-U, SMTP, SSH, SOCKS5, SIP, LDAP, SNMP, DNS-over-TCP
- input modes: demo facts, Unix socket, TCP socket
- Linux probe support: tracepoint, kprobe, tc ingress smoke/probe paths
- replay: deterministic for exported sessions
- DSL shape: pipeline-driven syntax preferred, structured and legacy key/value syntax still supported
- package shape: `gewy.pkg` manifest + `main.gewy` entry + pipeline `include(...)` expansion
- package deps: local path dependencies plus named sources via `dep.<name>=...`, `source.<name>=...`, and `include("std:file.gewy")`
- package resolution: `gewyc lock` emits a resolved `gewy.lock` snapshot
- language semantics: single-entry, function-unit DSL with no cross-file global mutable state
- workspace shape: `gewyvern` runtime crate + `gewyc` compiler CLI crate
- protocol registry shape: scanned gewy project packages under `protocols/`

## Road To 1.0

`gewyvern` is now on a deliberate release path:

- `v0.5.6` is the current stabilization point
- `v0.6.x` through `v0.9.x` should be used to close the remaining production gaps
- `v0.10.0` is intended to be the last pre-`1.0` release
- if the `1.0` gates are satisfied at `v0.10.0`, the project should move
  directly to `v1.0.0`

The goal is not “every protocol under the sun”. The `1.0.0` bar is that
`gewyvern` is trustworthy enough to serve as infra for process-level network
debugging: stable CLI/runtime behavior, stable DSL/IR/compiler boundaries,
reliable HTML/JSON reporting, and predictable operational performance.

The detailed milestone plan lives in [ROADMAP.md](/Users/Shared/chroot/dev/gewyvern/ROADMAP.md).

## Supported Protocol Families

- Web and secure transport:
  HTTP, HTTPS, TLS, QUIC, HTTP/3, Hysteria 2
- Name resolution and discovery:
  DNS, DNS-over-TCP, mDNS, SSDP
- Datagram and control protocols:
  STUN, CoAP, NTP, DHCP, WireGuard, SNMP, RADIUS, GTP-U, SIP
- Data stores and brokers:
  Redis, MQTT, PostgreSQL, MySQL, Memcached, AMQP
- Mail and directory services:
  SMTP, SSH, SOCKS5, LDAP

Most built-in packages model a concrete program-network path such as
request/response, auth/query, or publish/ack, rather than only matching a port
number.

## Workspace Layout

This repository is now easier to read as a workspace with clear responsibility
boundaries:

- [Cargo.toml](/Users/Shared/chroot/dev/gewyvern/Cargo.toml)
  Root workspace manifest. The `gewyvern` runtime crate lives at the workspace
  root and `crates/gewyc` is a separate compiler-facing CLI crate.
- [src](/Users/Shared/chroot/dev/gewyvern/src)
  Runtime, IR, DSL compiler front-end, export/replay, loader, and built-in CLI.
- [src/bin](/Users/Shared/chroot/dev/gewyvern/src/bin)
  Helper binaries such as socket senders used by local/runtime demos.
- [crates/gewyc](/Users/Shared/chroot/dev/gewyvern/crates/gewyc)
  Dedicated `.gewy` compiler CLI surface for binding, diagnostics, findings,
  stages, and envelope output.
- [dsl](/Users/Shared/chroot/dev/gewyvern/dsl)
  Built-in protocol and debugging DSL files. This is now the clearest place to
  see supported network-module behaviors.
- [protocols](/Users/Shared/chroot/dev/gewyvern/protocols)
  Registry-style gewy protocol packages. `gewyvern` scans `gewy.pkg` manifests
  here to register built-in protocols, entries, defaults, and aliases.
- `gewy.pkg`
  Package manifest for single-entry gewy projects; `gewyc` resolves this to the
  package `main.gewy` entry file.
- `gewy.lock`
  Resolved package snapshot emitted by `gewyc lock` for reproducible package
  inputs.
- [tests](/Users/Shared/chroot/dev/gewyvern/tests)
  TDD coverage for DSL compilation, runtime behavior, fragments, templates,
  socket input, and Linux smoke paths.
- [tests/support](/Users/Shared/chroot/dev/gewyvern/tests/support)
  Shared fact builders and test harness helpers.
- [docs](/Users/Shared/chroot/dev/gewyvern/docs)
  System, architecture, DSL, fragment, export, and development guides.
- [ebpf](/Users/Shared/chroot/dev/gewyvern/ebpf)
  Current hand-written eBPF fragment sources and smoke assets.
- [docker](/Users/Shared/chroot/dev/gewyvern/docker)
  Headless Linux dev/smoke environment support.
- [scripts](/Users/Shared/chroot/dev/gewyvern/scripts)
  Small helper scripts for demos and roundtrips.

## Main Entrypoints

- `cargo run -- ...`
  Start the main `gewyvern` runtime CLI for demos, DSL-driven sessions, socket
  ingest, findings, and JSON export.
- `cargo run -p gewyc -- ...`
  Compile or inspect `.gewy` files without starting a runtime session.
- `cargo run -p gewyc -- init my_app`
  Scaffold a minimal gewy package with `gewy.pkg`, `main.gewy`, and an included
  module file built around pure function-unit composition.
- `cargo run -p gewyc -- lock my_app`
  Resolve a gewy package manifest into a `gewy.lock` snapshot.
- `cargo test --workspace`
  Main regression path for the whole workspace.

## What Works In v0.5.6

- Fragment registry, attach planning, and attach reporting
- TDD-first runtime and rule specs
- Window-bounded sessions with `freeze(end)` and late-arrival cutoff
- Fact ingest gating based on real attach outcomes
- Rejected fact audit trail and aggregated summaries
- Transport flow reconstruction from packet/state/route/lineage facts
- Program flow reconstruction for process-aware network behavior
- shared flow-phase classification such as `resolve_route`,
  `initiate_connection`, `emit_payload`, and `emit_datagram`
- Deterministic reason chains for:
  - TCP handshake-oriented sessions
  - UDP datagram-oriented sessions
- DSL-driven protocol-path modeling for:
  - DNS over UDP
  - HTTP client/server request-response paths
  - TLS client paths
  - QUIC client initial paths
  - STUN binding exchanges
  - CoAP request/response exchanges
  - NTP client request/response exchanges
  - DHCP client discover/offer exchanges
  - WireGuard initiation/response handshake exchanges
  - mDNS query/response exchanges
  - SSDP discovery search/response exchanges
  - Redis RESP ping/pong exchanges
  - PostgreSQL connect, auth, simple-query/ready, and query-error exchanges
  - MQTT CONNECT/CONNACK exchanges
  - RADIUS Access-Request/Access-Accept exchanges
  - GTP-U Echo Request/Response exchanges
  - SMTP connect/banner/EHLO exchanges
  - SSH connect/banner/key-exchange-init exchanges
  - SOCKS5 method negotiation/connect-request/connect-success exchanges
  - HTTP CONNECT tunnel-request/tunnel-established exchanges
  - SIP REGISTER/200 OK exchanges
  - LDAP bind request/response exchanges
  - LDAP search request/result exchanges
  - LDAP bind + search directory session paths
  - SNMP GET/RESPONSE exchanges
  - DNS-over-TCP query/response exchanges
- Export/replay JSON including:
  - attach plan
  - attach report
  - debug summary
  - facts
  - transport flows
  - program flows
  - reasons
- Linux-only probe path for built-in fragments:
  - `tcp_state_fragment`
  - `route_meta_fragment`
  - `tcp_packet_meta_fragment`
  - UDP template attach path through `route_meta_fragment` + `udp_packet_meta_fragment`
- Structured `.gewy` block syntax lowered into the same compiler IR as the
  legacy key/value DSL shape
- `gewyc` as a separate workspace crate for compiler-facing workflows
- parameterized pure pipeline functions via `fn ...(...) { ... }` and `|> use(:fn_name, ...)`
- source-backed package dependency resolution and `gewy.lock` generation
- dynamic sampled payload offset support through fragment params and exported payload-byte maps

## Core Model

`Template` is now effectively:

```text
Template = Fragment Set + Window Profile + Reason Profile + Program Model
```

Where:

- `Fragment Set` decides what kernel/userland evidence can exist
- `Window Profile` decides session materialization bounds
- `Reason Profile` decides how L1/L3 reasoning is derived
- `Program Model` is the current embedded IR-like rule layer for materializing
  `program_flows`

The runtime pipeline today is:

```text
Template
  -> Fragment Registry
  -> Attach Planner
  -> Fact Stream
  -> Transport Flows
  -> Program Flows
  -> Reason Chains
  -> Export JSON
  -> Deterministic Replay
```

## Built-In Templates

- `handshake_debug`
  - `tcp_state_fragment`
  - `tcp_packet_meta_fragment`
  - `route_meta_fragment`
- `udp_debug`
  - `udp_packet_meta_fragment`
  - `route_meta_fragment`
- `udp_process_debug`
  - `udp_packet_meta_fragment`
  - `route_meta_fragment`
  - `sock_lineage_fragment`

## Repository Map

If you are orienting in the codebase, these files are the shortest path:

- [src/runtime.rs](/Users/Shared/chroot/dev/gewyvern/src/runtime.rs)
  Session lifecycle, ingest gating, finding synthesis, export assembly.
- [src/dsl.rs](/Users/Shared/chroot/dev/gewyvern/src/dsl.rs)
  `.gewy` parser/compiler front-end, including pipeline/function lowering,
  package entry resolution, and structured compatibility syntax.
- [src/fragment.rs](/Users/Shared/chroot/dev/gewyvern/src/fragment.rs)
  Fragment registry, capability surface, attach planning, and validation.
- [src/ir.rs](/Users/Shared/chroot/dev/gewyvern/src/ir.rs)
  Shared flow predicate and phase-kind logic that protocol DSLs compile onto.
- [src/export.rs](/Users/Shared/chroot/dev/gewyvern/src/export.rs)
  Replayable JSON bundle format and replay path.
- [src/gewyc.rs](/Users/Shared/chroot/dev/gewyvern/src/gewyc.rs)
  Shared compiler-facing report/envelope surface used by both CLIs.
- [crates/gewyc/src/main.rs](/Users/Shared/chroot/dev/gewyvern/crates/gewyc/src/main.rs)
  Dedicated compiler CLI.
- [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
  Best high-level system map after the README.

## DSL Files

The repository now includes first-class DSL files that compile into
`TemplateBinding` rather than into eBPF bytecode:

- [dsl/handshake_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/handshake_debug.gewy)
- [dsl/udp_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/udp_debug.gewy)
- [dsl/udp_process_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy)
- [dsl/pipeline_udp_process_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/pipeline_udp_process_debug.gewy)
- [dsl/structured_udp_process_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/structured_udp_process_debug.gewy)
- [dsl/dns_udp_process.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy)
- [dsl/https_connect_process.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/https_connect_process.gewy)
- [dsl/http_request_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy)
- [dsl/http_server_response_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http_server_response_path.gewy)
- [dsl/http3_request_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http3_request_path.gewy)
- [dsl/http3_server_response_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http3_server_response_path.gewy)
- [dsl/hy2_auth_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/hy2_auth_path.gewy)
- [dsl/hy2_tcp_relay_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/hy2_tcp_relay_path.gewy)
- [dsl/hy2_udp_relay_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/hy2_udp_relay_path.gewy)
- [dsl/tls_client_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy)
- [dsl/quic_client_initial_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/quic_client_initial_path.gewy)
- [dsl/quic_crypto_handshake_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/quic_crypto_handshake_path.gewy)
- [dsl/quic_stream_session_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/quic_stream_session_path.gewy)
- [dsl/quic_bidi_stream_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/quic_bidi_stream_path.gewy)
- [dsl/stun_binding_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/stun_binding_path.gewy)
- [dsl/coap_get_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/coap_get_path.gewy)
- [dsl/ntp_client_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ntp_client_path.gewy)
- [dsl/dhcp_client_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/dhcp_client_path.gewy)
- [dsl/wireguard_handshake_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/wireguard_handshake_path.gewy)
- [dsl/mdns_query_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mdns_query_path.gewy)
- [dsl/ssdp_discovery_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ssdp_discovery_path.gewy)
- [dsl/postgres_connect_process.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/postgres_connect_process.gewy)
- [dsl/postgres_auth_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/postgres_auth_path.gewy)
- [dsl/postgres_simple_query_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/postgres_simple_query_path.gewy)
- [dsl/postgres_query_error_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/postgres_query_error_path.gewy)
- [dsl/mysql_connect_process.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mysql_connect_process.gewy)
- [dsl/mysql_simple_query_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mysql_simple_query_path.gewy)
- [dsl/mysql_query_session.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_session.gewy)
- [dsl/mysql_query_error_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_error_path.gewy)
- [dsl/memcached_get_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/memcached_get_path.gewy)
- [dsl/memcached_set_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/memcached_set_path.gewy)
- [dsl/amqp_connection_start_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/amqp_connection_start_path.gewy)
- [dsl/amqp_basic_publish_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/amqp_basic_publish_path.gewy)
- [dsl/amqp_publish_session.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/amqp_publish_session.gewy)
- [dsl/redis_ping_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/redis_ping_path.gewy)
- [dsl/mqtt_connect_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mqtt_connect_path.gewy)
- [dsl/radius_access_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/radius_access_path.gewy)
- [dsl/gtpu_echo_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/gtpu_echo_path.gewy)
- [dsl/smtp_session_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/smtp_session_path.gewy)
- [dsl/ftp_session_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ftp_session_path.gewy)
- [dsl/ftp_passive_list_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ftp_passive_list_path.gewy)
- [dsl/ftp_retr_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ftp_retr_path.gewy)
- [dsl/ftp_stor_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ftp_stor_path.gewy)
- [dsl/ftp_active_list_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ftp_active_list_path.gewy)
- [dsl/ftp_active_retr_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ftp_active_retr_path.gewy)
- [dsl/ftp_active_stor_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ftp_active_stor_path.gewy)
- [dsl/ssh_session_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ssh_session_path.gewy)
- [dsl/ssh_auth_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ssh_auth_path.gewy)
- [dsl/ssh_auth_denied_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ssh_auth_denied_path.gewy)
- [dsl/ssh_channel_session_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ssh_channel_session_path.gewy)
- [dsl/socks5_session_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/socks5_session_path.gewy)
- [dsl/socks5_auth_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/socks5_auth_path.gewy)
- [dsl/socks5_auth_connect_denied_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/socks5_auth_connect_denied_path.gewy)
- [dsl/http_connect_auth_required_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_auth_required_path.gewy)
- [dsl/http_connect_authenticated_tunnel_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_authenticated_tunnel_path.gewy)
- [dsl/smtp_auth_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/smtp_auth_path.gewy)
- [dsl/smtp_mail_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/smtp_mail_path.gewy)
- [dsl/smtp_rcpt_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/smtp_rcpt_path.gewy)
- [dsl/smtp_data_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/smtp_data_path.gewy)
- [dsl/smtp_data_denied_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/smtp_data_denied_path.gewy)
- [dsl/smtp_rcpt_denied_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/smtp_rcpt_denied_path.gewy)
- [dsl/http_connect_tunnel_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http_connect_tunnel_path.gewy)
- [dsl/sip_register_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/sip_register_path.gewy)
- [dsl/ldap_bind_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_bind_path.gewy)
- [dsl/ldap_bind_denied_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_bind_denied_path.gewy)
- [dsl/ldap_search_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_search_path.gewy)
- [dsl/ldap_modify_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_path.gewy)
- [dsl/ldap_modify_denied_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_denied_path.gewy)
- [dsl/ldap_modify_constraint_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_constraint_path.gewy)
- [dsl/ldap_directory_session.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_session.gewy)
- [dsl/ldap_directory_write_session.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_write_session.gewy)
- [dsl/ldap_directory_sync_session.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_directory_sync_session.gewy)
- [dsl/snmp_get_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_path.gewy)
- [dsl/dns_tcp_query_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy)

These DSL files already cover the current built-in protocol/debugging shapes and
can express:

- fragment selection
- window profile selection
- reason profile selection
- program model operation/rules
- fragment parameter bindings
- template-local evidence tier overrides
- datagram predicates over direction, local/remote ports, minimum payload
  length, masked first-byte checks, fixed two-byte/four-byte prefixes, generic
  `byte_at:<offset>:<mask>:<value>` checks, and contiguous
  `bytes_at:<offset>:<byte>,<byte>,...` checks over currently sampled offsets
- a parallel QUIC packet predicate surface for `long_header` and QUIC packet
  `type` matching without falling back to raw UDP offset rules
- a parallel QUIC frame predicate surface for `crypto`, `ack`, `stream`,
  `datagram`, and `connection_close` frame-family matching
- packet predicates over direction, local/remote ports, masked first-byte checks,
  fixed four-byte prefixes, masked byte-4 checks, generic
  `byte_at:<offset>:<mask>:<value>` checks, and contiguous
  `bytes_at:<offset>:<byte>,<byte>,...` checks over currently sampled offsets
- binding diagnostics now surface unsupported payload offsets per rule, so a
  `.gewy` can explain why a `byte_at` or `bytes_at` matcher is outside the
  current fragment sampling surface
- package entry resolution through `gewy.pkg`
- single-entry pipeline projects that merge included module files into the
  package entry compile path
- function-unit pipeline composition through `fn ...() { ... }` plus `|> use(:fn_name)`
- parameterized pure function units through `fn ...(... ) { ... }` plus
  `|> use(:fn_name, ...)`
- no cross-file global mutable state; included files contribute pure DSL functions
- pipeline packages are merged into a single front-end module IR before lowering into `TemplateBinding`
- named package sources and source-backed dependency resolution in `gewy.pkg`
- resolved package lock snapshots through `gewyc lock`
- dynamic sampled payload offsets via fragment params in addition to built-in
  offset defaults

In normal runtime use, [protocols](/Users/Shared/chroot/dev/gewyvern/protocols)
is now the main operational surface. `--protocol`, `--entry`, and `--scan-all`
resolve through scanned `gewy.pkg` manifests first, and only fall back to
static compatibility entries when necessary.

The shared datagram predicate surface is what the current UDP-family protocol
DSLs build on. In practice, the engine is already using the same IR layer to
differentiate:

- generic UDP process activity
- DNS request/reply paths
- QUIC initial and handshake traffic
- QUIC packet-family modeling through a dedicated parallel IR surface
- Hysteria 2 auth-shaped QUIC stream paths
- Hysteria 2 UDP relay paths over QUIC datagram frames
- STUN binding request/response pairs
- GTP-U echo request/response pairs
- CoAP request/response pairs
- NTP client request/response pairs
- DHCP client discover/offer pairs
- WireGuard initiation/response pairs
- mDNS query/response pairs
- SSDP discovery search/response pairs
- Redis RESP ping/pong pairs
- MQTT CONNECT/CONNACK pairs
- RADIUS Access-Request/Access-Accept pairs
- SMTP connect/banner/EHLO sequences
- SSH connect/banner/key-exchange-init sequences
- SOCKS5 method-selection/connect-request/connect-success sequences
- HTTP CONNECT tunnel-request/tunnel-established sequences
- SIP REGISTER/200 OK pairs
- LDAP bind request/response pairs
- LDAP search request/result pairs
- LDAP bind + search directory-session paths
- SNMP GET/RESPONSE pairs
- DNS-over-TCP query/response pairs

## Development

This repository is intentionally TDD-driven.

Main commands:

- `cargo tdd`
- `cargo tdd-one <test_name>`
- `cargo tdd-rules`
- `cargo test`
- `cargo run -p gewyc -- <path.gewy>`
- `cargo run -p gewyc -- diagnostics <path.gewy> --json`
- `cargo run -p gewyc -- findings <path.gewy> --json`
- `cargo run -p gewyc -- stages <path.gewy> --json`
- `cargo run -p gewyc -- envelope <path.gewy> --json`
- `cargo run -p gewyc -- <path.gewy> --emit diagnostics --json --out /tmp/gewyc.json`
- `cargo run -p gewyc -- <path.gewy> --emit envelope --json --out /tmp/gewyc-envelope.json`
- `cargo test benchmark_summary_json_large_protocol_flow_export -- --ignored --nocapture`
- `cargo test benchmark_summary_line_large_protocol_flow_export -- --ignored --nocapture`

`gewyc stages` now includes a validation summary for payload-byte support:

- `sampled_payload_offsets`: offsets currently exposed by the selected fragment set
- `required_payload_offsets`: offsets referenced by the binding's offset-based DSL predicates
- `unsupported_payload_offsets`: required offsets that current fragments do not sample

Unlike `binding` or `diagnostics`, `stages` now keeps partial compiler output on
parse and validation failures, so stage-local findings are still available when
a `.gewy` does not fully compile. Only outer file I/O failures still surface as
a top-level command error.

Current test layers:

- [tests/runtime_tdd.rs](/Users/Shared/chroot/dev/gewyvern/tests/runtime_tdd.rs)
- [tests/template_rules_tdd.rs](/Users/Shared/chroot/dev/gewyvern/tests/template_rules_tdd.rs)
- [tests/fragment_rules_tdd.rs](/Users/Shared/chroot/dev/gewyvern/tests/fragment_rules_tdd.rs)
- [tests/linux_smoke_tdd.rs](/Users/Shared/chroot/dev/gewyvern/tests/linux_smoke_tdd.rs)

Current benchmark entrypoints:

- summary JSON rendering over many matched protocol flows
- summary line rendering over many matched protocol flows

These are lightweight ignored tests today, so they run without adding a
separate benchmark harness dependency.

## Quick Start

Run the built-in demos:

```bash
cargo run
cargo run -- --demo tcp
cargo run -- --demo udp
cargo run -- --findings
cargo run -- --demo both --json
cargo run -- --demo both --json --summary-only
cargo run -- --findings --json
```

Recommended operational paths:

```bash
# one built-in protocol path
cargo run -- --protocol quic --entry bidi --json --summary-only

# one process-scoped protocol path
cargo run -- --protocol hy2 --entry tcp --pid 4242 --json

# sweep the built-in protocol registry
cargo run -- --scan-all --json --summary-only
```

Run a DSL-driven demo:

```bash
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/https_connect_process.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/quic_client_initial_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/stun_binding_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/coap_get_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/ntp_client_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/dhcp_client_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/wireguard_handshake_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/mdns_query_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/ssdp_discovery_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/mysql_connect_process.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/mysql_simple_query_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/mysql_query_session.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/memcached_get_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/amqp_connection_start_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/amqp_basic_publish_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/amqp_publish_session.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_ping_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/mqtt_connect_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/radius_access_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/smtp_session_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/ftp_session_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/ftp_passive_list_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/ftp_retr_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/ftp_stor_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/ftp_active_list_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/ftp_active_retr_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/ftp_active_stor_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy --json --summary-only
```

Lock onto a built-in protocol model directly:

```bash
cargo run -- --list-protocols
cargo run -- --list-protocols --json
cargo run -- --list-entries mysql
cargo run -- --list-entries ldap --json
cargo run -- --protocol mysql --json --summary-only
cargo run -- --protocol amqp --findings --json
```

Lock output to one process PID:

```bash
cargo run -- --protocol mysql --pid 4242 --json
cargo run -- --protocol ldap --entry sync --pid 4242 --findings --json
```

Run a full protocol sweep with the built-in default protocol set or a custom
set file or registry directory:

```bash
cargo run -- --scan-all --json --summary-only
cargo run -- --scan-all --findings --json
cargo run -- --scan-all --protocol-set /tmp/protocols.txt --json --summary-only
cargo run -- --scan-all --protocol-set /Users/Shared/chroot/dev/gewyvern/protocols --json --summary-only
cargo run -- --scan-all --summary-only --report-format html --out /tmp/gewyvern-scan-report.html
cargo run -- --scan-all --findings --report-format html --out /tmp/gewyvern-scan-findings.html
```

`--scan-all` now walks every registered protocol entry under `protocols/`, not
just one default entry per protocol. The JSON report includes top-level scan
counts plus per-target `protocol_flows` and `process_network_profiles`; the
HTML report renders the same scan as a visual summary page.

Example protocol set file:

```text
# default entry
mysql

# explicit entry
amqp:publish
ldap bind
```

Built-in protocol discovery now comes from scanning gewy project manifests in
[protocols](/Users/Shared/chroot/dev/gewyvern/protocols). Each package uses a
`gewy.pkg` like:

```text
name=mysql_session
version=0.5.6
entry=main.gewy
register.protocol=mysql
register.entry=session
register.default=true
register.aliases=mysql-session,mysql_session
```

Select a specific gewy entry mode for one protocol:

```bash
cargo run -- --protocol mysql --entry connect --json
cargo run -- --protocol mysql --entry session --json
cargo run -- --protocol amqp --entry start --json
cargo run -- --protocol amqp --entry session --findings --json
```

Inspect binding diagnostics without starting a runtime session:

```bash
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --diagnostics
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --diagnostics --json
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy
cargo run -p gewyc -- diagnostics /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- findings /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- stages /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --emit diagnostics --json --out /tmp/gewyc-diagnostics.json
```

## `gewyc`

`gewyc` is the first extracted DSL toolchain surface for `.gewy`.

Current responsibilities:

- compile `.gewy` into validated `TemplateBinding`
- print compiled binding in text or JSON
- print binding diagnostics in text or JSON
- print structured compiler findings in text or JSON
- print staged compiler output in text or JSON
- write compiler output to a file with `--out`
- select compiler surface explicitly with `--emit binding|diagnostics|findings|stages`

Current examples:

```bash
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/redis_ping_path.gewy
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/redis_ping_path.gewy --json
cargo run -p gewyc -- diagnostics /Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy
cargo run -p gewyc -- diagnostics /Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy --json
cargo run -p gewyc -- findings /Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy --json
cargo run -p gewyc -- stages /Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy --json
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy --emit diagnostics --json --out /tmp/dns-tcp-diagnostics.json
```

Inspect the most suspicious program/network modules directly:

```bash
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --findings
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --findings --json
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy --findings
```

Write output to a file:

```bash
cargo run -- --demo udp --json --out /tmp/gewyvern-export.json
cargo run -- --demo udp --json --summary-only --out /tmp/gewyvern-summary.jsonl
```

## Socket Input

`gewyvern` can ingest fact JSON Lines over Unix or TCP sockets.

Unix socket:

```bash
cargo run -- --unix-socket /tmp/gewyvern.sock --template udp --json
cargo run --bin gewyvern_socket_send -- --socket /tmp/gewyvern.sock --template udp
```

TCP socket:

```bash
cargo run -- --tcp-socket 127.0.0.1:9000 --template udp --json
cargo run --bin gewyvern_socket_send -- --tcp-socket 127.0.0.1:9000 --template udp
```

Socket session from a DSL file:

```bash
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --unix-socket /tmp/gewyvern.sock --json
```

Socket session locked to a built-in protocol and PID:

```bash
cargo run -- --protocol mysql --entry session --pid 4242 --tcp-socket 127.0.0.1:9000 --json
```

Socket session scanned against the default protocol set or a custom set file:

```bash
cargo run -- --scan-all --pid 4242 --tcp-socket 127.0.0.1:9000 --json --summary-only
cargo run -- --scan-all --protocol-set /tmp/protocols.txt --tcp-socket 127.0.0.1:9000 --findings --json
cargo run -- --scan-all --pid 4242 --tcp-socket 127.0.0.1:9000 --summary-only --report-format html --out /tmp/gewyvern-socket-scan.html
```

Remote TCP listeners are now opt-in:

```bash
cargo run -- --protocol mysql --entry session --tcp-socket 0.0.0.0:9000 --socket-trust unsafe-remote --json
cargo run -- --protocol mysql --entry session --tcp-socket 0.0.0.0:9000 --allow-remote-socket --json
```

`--socket-trust trusted-local` is the default. `--allow-remote-socket` remains as a
compatibility shorthand for `--socket-trust unsafe-remote`.
Rendered summaries and full JSON exports carry this as `ingest_trust_mode`.

Serve multiple sessions:

```bash
cargo run -- --tcp-socket 127.0.0.1:9000 --template udp --serve --json --summary-only
cargo run -- --tcp-socket 127.0.0.1:9000 --template udp --serve --max-sessions 2 --json
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --serve --max-sessions 2 --json --summary-only
```

Roundtrip demo:

```bash
bash scripts/socket_roundtrip_demo.sh /tmp/gewyvern.sock udp /tmp/gewyvern-out.json unix
bash scripts/socket_roundtrip_demo.sh 127.0.0.1:9000 udp /tmp/gewyvern-out.json tcp
```

## Linux eBPF Probe Environment

The repo includes a headless Linux path for real probe smoke tests.

Build and enter the container:

```bash
docker compose -f docker-compose.headless-linux.yml build
docker compose -f docker-compose.headless-linux.yml up -d
docker compose -f docker-compose.headless-linux.yml exec ebpf-dev bash
```

Inside Linux, useful commands are:

```bash
cargo tdd
cargo linux-smoke
```

## Important Current Boundaries

- This is still a prototype, not a stable public schema/runtime
- eBPF programs are still hand-written C, not generated from IR
- `ProgramModel` is now DSL-driven for the built-in path templates, but the IR
  surface is still intentionally small and evolving
- `ProgramFlow.operation` can now carry template-defined custom ids, but the rule
  surface is still intentionally small
- the intended DSL compile target is `template + fragment params`, not eBPF bytecode
- protocol specialization is currently biased toward flow evidence that can be
  expressed through the shared datagram/socket/route IR, not arbitrary payload
  parsers in kernel space
- Program-flow reconstruction is still intentionally small and conservative
- Local Unix/TCP socket live tests are ignored in restricted environments that
  do not allow bind permissions
- `tc egress` is not implemented yet in the real Linux probe path

## Repo Docs

- [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
- [docs/walkthrough.md](/Users/Shared/chroot/dev/gewyvern/docs/walkthrough.md)
- [docs/overview.md](/Users/Shared/chroot/dev/gewyvern/docs/overview.md)
- [docs/architecture.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture.md)
- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
- [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)
- [docs/export-format.md](/Users/Shared/chroot/dev/gewyvern/docs/export-format.md)
- [docs/headless-linux.md](/Users/Shared/chroot/dev/gewyvern/docs/headless-linux.md)

## Near-Term Direction

The next meaningful step after `v0.5.6` is not only “more protocol branches”.
It is continuing to make the DSL and IR more explicit, so protocol behavior is
described as program-network-module structure rather than as a pile of
protocol-specific special cases, while steadily closing the remaining gaps on
the path to `v1.0.0`. The concrete release path is tracked in
[ROADMAP.md](/Users/Shared/chroot/dev/gewyvern/ROADMAP.md).
