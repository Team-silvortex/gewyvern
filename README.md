# gewyvern v0.4

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

## Status

- project version: `0.4.0`
- stage: working prototype with a stabilized internal workspace layout
- transport support: TCP + UDP
- protocol path coverage in DSL: DNS, HTTP, TLS, QUIC, STUN, CoAP, NTP, DHCP, WireGuard, mDNS, SSDP, Redis, MQTT, PostgreSQL, RADIUS, GTP-U, SMTP, SIP, LDAP, SNMP, DNS-over-TCP
- input modes: demo facts, Unix socket, TCP socket
- Linux probe support: tracepoint, kprobe, tc ingress smoke/probe paths
- replay: deterministic for exported sessions
- DSL shape: pipeline-driven syntax preferred, structured and legacy key/value syntax still supported
- workspace shape: `gewyvern` runtime crate + `gewyc` compiler CLI crate

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
- `cargo test --workspace`
  Main regression path for the whole workspace.

## What Works In v0.4

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
  `.gewy` parser/compiler front-end, including structured block syntax lowering.
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
- [dsl/tls_client_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/tls_client_path.gewy)
- [dsl/quic_client_initial_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/quic_client_initial_path.gewy)
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
- [dsl/redis_ping_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/redis_ping_path.gewy)
- [dsl/mqtt_connect_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/mqtt_connect_path.gewy)
- [dsl/radius_access_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/radius_access_path.gewy)
- [dsl/gtpu_echo_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/gtpu_echo_path.gewy)
- [dsl/smtp_session_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/smtp_session_path.gewy)
- [dsl/sip_register_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/sip_register_path.gewy)
- [dsl/ldap_bind_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/ldap_bind_path.gewy)
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
  length, masked first-byte checks, fixed two-byte/four-byte prefixes, and
  generic `byte_at:<offset>:<mask>:<value>` checks over currently sampled
  offsets
- packet predicates over direction, local/remote ports, masked first-byte checks,
  fixed four-byte prefixes, masked byte-4 checks, and generic
  `byte_at:<offset>:<mask>:<value>` checks over currently sampled offsets
- binding diagnostics now surface unsupported payload offsets per rule, so a
  `.gewy` can explain why a `byte_at` matcher is outside the current fragment
  sampling surface

The shared datagram predicate surface is what the current UDP-family protocol
DSLs build on. In practice, the engine is already using the same IR layer to
differentiate:

- generic UDP process activity
- DNS request/reply paths
- QUIC initial and handshake traffic
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
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_ping_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/mqtt_connect_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/radius_access_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/smtp_session_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy --json --summary-only
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

Serve multiple sessions:

```bash
cargo run -- --tcp-socket 127.0.0.1:9000 --template udp --serve --json --summary-only
cargo run -- --tcp-socket 127.0.0.1:9000 --template udp --serve --max-sessions 2 --json
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

The next meaningful step after `v0.4` is not only “more protocol branches”.
It is continuing to make the DSL and IR more explicit, so protocol behavior is
described as program-network-module structure rather than as a pile of
protocol-specific special cases.
