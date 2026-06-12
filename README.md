# gewyvern v1.4.0

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

- project version: `1.4.0`
- stage: active `1.4.x` line focused on protocol depth, compiler ergonomics,
  runtime/report stability, and cleaner collaboration surfaces across
  `gewyvern`, `etragon`, and `leserpent`
- transport support: TCP + UDP
- protocol path coverage in DSL: DNS, HTTP, TLS, QUIC, STUN, CoAP, NTP, DHCP, WireGuard, mDNS, SSDP, Redis, MQTT, PostgreSQL, MySQL, Memcached, AMQP, RADIUS, GTP-U, SMTP, SSH, SOCKS5, SIP, LDAP, SNMP, RTSP, DNS-over-TCP
- input modes: demo facts, Unix socket, TCP socket
- Linux probe support: tracepoint, kprobe, tc ingress smoke/probe paths
- replay: deterministic for exported sessions
- DSL shape: pipeline-driven `gewylang` stable subset
- package shape: `gewy.pkg` manifest + `main.gewy` entry + pipeline `include(...)` expansion
- package deps: local path dependencies plus named sources via `dep.<name>=...`, `source.<name>=...`, and `include("std:file.gewy")`
- package resolution: `gewyc lock` emits a resolved `gewy.lock` snapshot
- language semantics: single-entry, function-unit DSL with no cross-file global mutable state
- preferred gewylang subset: expression-style function units, local immutable `let`, positional `use(...)`, and package-entry composition
- workspace shape: `gewyvern` runtime crate + `gewyc` compiler CLI crate
- protocol registry shape: scanned gewy project packages under `protocols/`

## Current Release Line

`gewyvern` is no longer a pre-`1.0` convergence story. The current line is:

- historical validation baseline: `v0.10.0`
- current release line: `v1.4.0`
- current focus: deepen protocol quality, keep runtime/report/compiler behavior
  predictable, and make cross-project collaboration (`gewyvern` + `etragon` +
  `leserpent`) more deliberate without bloating the standalone debugger core
- next likely work line: `v1.5.x`, unless a later architectural break justifies
  a deliberately chosen `v2.0`

The goal is still not “every protocol under the sun”. The `1.x` bar is that
`gewyvern` is trustworthy enough to serve as infra for process-level network
debugging: stable CLI/runtime behavior, stable DSL/compiler boundaries,
reliable HTML/JSON reporting, and predictable operational performance.

The detailed milestone plan lives in [ROADMAP.md](/Users/Shared/chroot/dev/gewyvern/ROADMAP.md).

For the shorter statement of what the current `v1.4.0` line should already
feel like in practice, see
[docs/v1.4-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v1.4-posture.md).

For the historical minor-line record starting with the deliberate `v0.13.x`
convergence line, see
[docs/history/index.md](/Users/Shared/chroot/dev/gewyvern/docs/history/index.md).
That page now also keeps a compact release-line ledger for `v0.10.0`,
`v0.13.x`, `v1.4.x`, and the next reserved minor slot.

For the narrow machine-facing contract that downstream automation, sidecars,
and enrich/rerank pipelines should consume, see
[docs/machine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/machine-contract.md).

For the two core compiler/runtime contract shelves that now anchor protocol
authoring and IR review, see:

- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
  Canonical protocol families, entries, aliases, defaults, and registry/CLI
  resolution.
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
  Lowered `program_model` / `reason_model` shape, `ir_lowering_delta`, and
  `gewyc explain --focus ir` contract candidate.

For the dedicated note on how nearby sidecars such as `etragon` are surfaced
as additive collaboration context, see
[docs/sidecar-collaboration.md](/Users/Shared/chroot/dev/gewyvern/docs/sidecar-collaboration.md).

For long-lived `--serve` / API / external-engine operational behavior, see
[docs/service-behavior.md](/Users/Shared/chroot/dev/gewyvern/docs/service-behavior.md).

For the dedicated note on standalone runtime boundaries, exposure posture, and
what `gewyvern` should not be treated as in the current `1.4.x` line, see
[docs/security-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/security-posture.md).

For the concrete field-validation matrix and local smoke entrypoint,
see [docs/field-validation.md](/Users/Shared/chroot/dev/gewyvern/docs/field-validation.md).

For the shorter running record of what packaged/runtime validation has already
demonstrated, see [docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md).

The current narrow prelaunch protocol/IR scope now lives in
[docs/field-validation.md](/Users/Shared/chroot/dev/gewyvern/docs/field-validation.md),
and the already-visible postlaunch follow-ups now live in
[docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md).

For protocol-registry drift specifically, use
[scripts/registry_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/registry_validation.sh)
to validate scanned gewy packages one by one.

For live standalone `--serve` plus read-only API validation, use
[scripts/runtime_operator_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/runtime_operator_validation.sh).

For concentrated high-frequency protocol and mixed-flow checks, use
[scripts/high_frequency_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/high_frequency_validation.sh).

For native Linux packaging layout, DEB control metadata, and RPM spec
generation, see [docs/packaging.md](/Users/Shared/chroot/dev/gewyvern/docs/packaging.md).

## Supported Protocol Families

- Web, secure transport, and modern proxying:
  HTTP, HTTPS, TLS, QUIC, HTTP/3, Hysteria 2
- Name resolution and discovery:
  DNS, DNS-over-TCP, mDNS, SSDP
- Datagram and control protocols:
  STUN, CoAP, NTP, DHCP, WireGuard, SNMP, RADIUS, GTP-U, SIP
- Data stores, brokers, and cache access:
  Redis, MQTT, PostgreSQL, MySQL, Memcached, AMQP
- Mail, directory, file-transfer, and remote access:
  SMTP, IMAP, POP3, FTP, SSH, SOCKS5, LDAP, Kerberos, RTSP

Most built-in entries model a concrete program-network path such as
request/response, auth/query, relay setup, or publish/ack, rather than only
matching a port number.

## Repository Shape

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
- [packaging](/Users/Shared/chroot/dev/gewyvern/packaging)
  Native Linux packaging templates for DEB/RPM metadata.

## Documentation Entrypoints

Use the docs in two layers:

- [docs/index.md](/Users/Shared/chroot/dev/gewyvern/docs/index.md)
  The durable top-level map for project, runtime, DSL, validation, and
  packaging pages.
- [docs/book/index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/index.md)
  The structured reading spine for tutorials, how-to, reference, and
  explanation.

If you only want the project's current core contract surfaces, start with:

- [docs/machine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/machine-contract.md)
  Narrow machine-facing runtime and analysis contract.
- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
  Protocol family/entry shelf contract.
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
  Compiler IR lowering contract.

If you are orienting yourself for the first time, the shortest useful order is:

1. [README.md](/Users/Shared/chroot/dev/gewyvern/README.md)
2. [docs/index.md](/Users/Shared/chroot/dev/gewyvern/docs/index.md)
3. [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
4. [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
5. [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)

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
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/build_packages.sh --layout-only`
  Stage the Linux install tree and render DEB/RPM metadata without requiring
  host package-manager tools.
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/build_packages_in_container.sh --format all`
  Build real `.deb` and `.rpm` artifacts inside the bundled Linux container.
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/package_install_smoke.sh`
  Install the latest local `.deb` and `.rpm` into clean Linux containers and
  verify the packaged binaries and assets come up correctly.
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/container_runtime_validation.sh`
  Install the latest local native packages into clean Linux containers and run
  a real packaged `--serve` plus API validation path.
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/release_container_check.sh`
  Run the current release-oriented packaged Linux validation suite from one
  entrypoint, covering install smoke, packaged runtime validation, and the
  packaged protocol/operator summary checks.
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/container_validation_summary.sh`
  Run the packaged Linux container validation suite from one entrypoint,
  covering both packaged protocol validation and packaged operator-path
  validation.
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/container_protocol_validation.sh`
  Install the latest local native packages into clean Linux containers and
  verify packaged high-frequency protocol support across DNS, HTTP, TLS,
  HTTP/3, QUIC, SSH, SOCKS5, MySQL, PostgreSQL, SMTP, LDAP, Redis, MQTT,
  AMQP, RADIUS, SNMP, FTP, IMAP, POP3, Kerberos, RTSP, plus `--scan-all`.
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/container_operator_path_validation.sh`
  Install the latest local native packages into clean Linux containers and
  verify packaged operator-path chains for `DNS -> QUIC -> HTTP/3`,
  `DNS -> TLS -> HTTPS CONNECT`, `DNS -> SOCKS5 -> HTTPS CONNECT`,
  `DNS -> TLS -> Postgres`, `DNS -> TLS -> MySQL`, `DNS -> TLS -> SMTP auth`,
  `DNS -> SMTP`, and a conservative `SOCKS5 auth denied` negative-path guard.
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/three_module_stack_smoke.sh`
  Build local Linux binaries for `gewyvern` and `etragon`, start two nearby
  `gewyvern` instances in Docker, attach one resident `etragon` sidecar to one
  runtime, then register both runtimes into a live `leserpent` control plane
  and verify the resulting fleet summary plus one directly observed paired
  sidecar runtime. This is the current multi-instance collaboration smoke for
  the `gewyvern + etragon + leserpent` stack.

## What Works In The Current Pre-1.0 Line

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
  - SIP INVITE/200 OK exchanges
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
- Pipeline `.gewy` syntax lowered into the current compiler IR
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
  package entry resolution, and stable-subset `gewylang` parsing.
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

## DSL Coverage

The repository now includes first-class DSL files that compile into
`TemplateBinding` rather than into eBPF bytecode:

- Debug and compiler baselines:
  [dsl/handshake_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/handshake_debug.gewy),
  [dsl/udp_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/udp_debug.gewy),
  [dsl/udp_process_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy),
  [dsl/pipeline_udp_process_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/pipeline_udp_process_debug.gewy),
  [dsl/structured_udp_process_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/structured_udp_process_debug.gewy)
- Web, secure transport, and proxying:
  HTTP request/response and CONNECT,
  HTTPS connect,
  TLS client,
  QUIC initial/crypto/stream/bidi,
  HTTP/3 request/server response,
  HY2 auth/UDP relay/TCP relay
- Name resolution and discovery:
  DNS over UDP and TCP,
  mDNS,
  SSDP
- Datagram and control protocols:
  STUN,
  CoAP,
  NTP,
  DHCP,
  WireGuard,
  SNMP,
  RADIUS,
  GTP-U,
  SIP
- Data stores and brokers:
  PostgreSQL connect/auth/query/session/error,
  MySQL connect/query/session/error,
  Redis ping/session/get/set,
  Memcached get/set,
  MQTT connect/publish/subscribe/pubrec/pubrel/pubcomp/disconnect,
  AMQP start/publish/session/consume
- Mail, directory, and access protocols:
  SMTP session/auth/mail/rcpt/data and denied branches,
  IMAP auth/auth-denied/select mailbox branches,
  POP3 auth/auth-denied/list mailbox branches,
  FTP session/list/retr/stor across passive and active modes,
  SSH session/auth/channel branches,
  SOCKS5 session/auth/denied branches,
  LDAP bind/search/modify/session/write/sync and denied branches,
  Kerberos AS/TGS and KRB-ERROR branches,
  RTSP options/describe/setup signaling branches

The clearest operational view of those built-ins is the registry under
[protocols](/Users/Shared/chroot/dev/gewyvern/protocols). Each package there
maps one protocol entry to its `main.gewy`, default/alias metadata, and scan
registration shape.

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
- SIP INVITE/200 OK pairs
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
- `cargo run -p gewyc -- explain <path.gewy>`
- `cargo run -p gewyc -- explain <path.gewy> --focus binding`
- `cargo run -p gewyc -- explain <path.gewy> --focus validation`
- `cargo run -p gewyc -- frontend <path.gewy> --json`
- `cargo run -p gewyc -- frontend <path.gewy> --focus graph`
- `cargo run -p gewyc -- frontend <path.gewy> --focus expansion`
- `cargo run -p gewyc -- diagnostics <path.gewy> --json`
- `cargo run -p gewyc -- findings <path.gewy> --json`
- `cargo run -p gewyc -- stages <path.gewy> --json`
- `cargo run -p gewyc -- envelope <path.gewy> --json`
- `cargo run -p gewyc -- <path.gewy> --emit diagnostics --json --out /tmp/gewyc.json`
- `cargo run -p gewyc -- <path.gewy> --emit envelope --json --out /tmp/gewyc-envelope.json`
- `cargo test benchmark_summary_json_large_protocol_flow_export -- --ignored --nocapture`
- `cargo test benchmark_summary_line_large_protocol_flow_export -- --ignored --nocapture`
- `cargo test benchmark_analysis_snapshot_large_protocol_flow_export -- --ignored --nocapture`
- `cargo test benchmark_analysis_snapshot_json_large_protocol_flow_export -- --ignored --nocapture`
- `cargo test benchmark_findings_json_large_protocol_flow_export -- --ignored --nocapture`
- `cargo test benchmark_scan_report_json_large_protocol_flow_export -- --ignored --nocapture`
- `cargo test benchmark_scan_report_text_large_protocol_flow_export -- --ignored --nocapture`
- `cargo test benchmark_scan_report_html_large_protocol_flow_export -- --ignored --nocapture`
- `cargo test benchmark_http_transactions_json_large_view -- --ignored --nocapture`
- `cargo test benchmark_http_transactions_text_large_view -- --ignored --nocapture`
- `cargo test benchmark_gewyc_ -- --ignored --nocapture`
- `bash scripts/benchmark_summary.sh`
- `bash scripts/benchmark_summary.sh 3 benchmark_scan_report_`
- `bash scripts/benchmark_summary.sh 3 benchmark_gewyc_`

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

- analysis snapshot construction over many matched protocol flows
- analysis snapshot JSON rendering over many matched protocol flows
- findings JSON rendering over many matched protocol flows
- summary JSON rendering over many matched protocol flows
- summary line rendering over many matched protocol flows
- scan report JSON rendering over many large targets
- scan report text rendering over many large targets
- scan report HTML rendering over many large targets
- HTTP transaction JSON rendering over a large composed transaction view
- HTTP transaction text rendering over a large composed transaction view

These are lightweight ignored tests today, so they run without adding a
separate benchmark harness dependency.

For less noisy local measurements, `scripts/benchmark_summary.sh` will run the
selected benchmark filter multiple times and print `min / median / max / avg`
for each benchmark line it sees.

Current local reference medians are tracked in
[performance-baselines.md](/Users/Shared/chroot/dev/gewyvern/docs/performance-baselines.md).
That page also serves as the current `1.4.x` acceptance baseline for the main
hot paths.

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
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/postgres_query_session.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/memcached_get_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/amqp_connection_start_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/amqp_basic_publish_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/amqp_publish_session.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/amqp_basic_consume_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_ping_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_session_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_get_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_set_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_del_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_incr_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_decr_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_mget_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_mset_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_exists_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_expire_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_ttl_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_pttl_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_hget_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_hset_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_hmget_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_hmset_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_lpush_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_rpush_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_lpop_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_rpop_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_sadd_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_smembers_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_zadd_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_zrange_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_zrem_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_zcard_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_zscore_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_zrank_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_zrevrank_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_zrangebyscore_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_zrevrangebyscore_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_zincrby_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_zcount_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_zpopmin_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_zpopmax_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_bzpopmin_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_bzpopmax_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_publish_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_subscribe_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_xadd_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_xread_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_xrange_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_xrevrange_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_xdel_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_xtrim_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/redis_xlen_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/mqtt_connect_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/mqtt_publish_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/mqtt_subscribe_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/mqtt_pubrec_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/mqtt_pubrel_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/mqtt_pubcomp_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/mqtt_disconnect_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/sip_bye_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/rtsp_play_path.gewy --json --summary-only
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
version=0.10.0
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
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus binding
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus validation
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus graph
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --focus expansion
cargo run -p gewyc -- diagnostics /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- findings /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- stages /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --json
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --emit diagnostics --json --out /tmp/gewyc-diagnostics.json
```

## `gewyc`

`gewyc` is the first extracted DSL toolchain surface for `.gewy`.

Current responsibilities:

- compile `.gewy` into validated `TemplateBinding`
- explain parse/front-end/validation/diagnostics/findings in one human-oriented surface
- inspect the standalone pipeline frontend summary before binding or validation
- print compiled binding in text or JSON
- print binding diagnostics in text or JSON
- print compiler findings in text or JSON
- print staged compiler output in text or JSON
- write compiler output to a file with `--out`
- select compiler surface explicitly with `--emit binding|explain|frontend|diagnostics|findings|stages|envelope`

Current examples:

```bash
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/redis_ping_path.gewy
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/redis_ping_path.gewy --json
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy --focus binding
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy --focus parse
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy --json
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy --focus includes
cargo run -p gewyc -- frontend /Users/Shared/chroot/dev/gewyvern/dsl/dns_tcp_query_path.gewy --focus expansion
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

Socket session focused on one built-in protocol:

```bash
cargo run -- --protocol mysql --entry session --tcp-socket 127.0.0.1:9000 --json
```

Socket session scanned against the default protocol set or a custom set file:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --json --summary-only
cargo run -- --scan-all --protocol-set /tmp/protocols.txt --tcp-socket 127.0.0.1:9000 --findings --json
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --summary-only --report-format html --out /tmp/gewyvern-socket-scan.html
```

Human-friendly ingest modes are now the primary interface:

```bash
cargo run -- --protocol mysql --entry session --tcp-socket 127.0.0.1:9000 --ingest-mode local-advisory --json
cargo run -- --protocol mysql --entry session --tcp-socket 0.0.0.0:9000 --ingest-mode remote-advisory --json
```

Mode meanings:

- `local-advisory`
  local socket ingest, process-level conclusions are advisory because lineage is
  still unverified
- `remote-advisory`
  remote socket ingest, explicitly opt-in and still unverified

Legacy compatibility aliases still work:

- `--socket-trust trusted-local|unsafe-remote`
- `--allow-remote-socket`

Rendered summaries and JSON reports now carry both:

- `ingest_mode`
- `ingest_trust_mode`

`--pid` is intentionally rejected with socket ingest, because unverified socket
lineage should not be presented as strong PID-scoped attribution.

Serve multiple sessions:

```bash
cargo run -- --tcp-socket 127.0.0.1:9000 --template udp --serve --json --summary-only
cargo run -- --tcp-socket 127.0.0.1:9000 --template udp --serve --max-sessions 2 --json
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --serve --max-sessions 2 --json --summary-only
```

Expose the latest serve-session snapshot over a read-only HTTP API:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --serve --api-socket 127.0.0.1:9100 --json --summary-only
```

Useful API endpoints:

- `/health`
- `/v1/capabilities`
- `/v1/latest/meta`
- `/v1/latest/targets`
- `/v1/latest/summary.txt`
- `/v1/latest/summary.json`
- `/v1/latest/findings.json`
- `/v1/latest/analysis.json`
- `/v1/latest/export.json`
- `/v1/latest/report.json`
- `/v1/latest/report.html`
- `/v1/latest/targets/<name>/summary.json`
- `/v1/latest/targets/<name>/findings.json`
- `/v1/latest/targets/<name>/analysis.json`
- `/v1/latest/targets/<name>/export.json`
- `/v1/latest/targets/<name>/report.json`
- `/v1/latest/targets/<name>/report.html`

`--api-socket` is intended for `--serve` mode and exposes only the latest
session or scan snapshot in memory so that other local services can consume
`gewyvern` results without parsing terminal output.

If another service wants a composable intermediate analysis result instead of a
fully rendered report, prefer `analysis.json`. It exposes the target-level
analysis snapshot directly: `protocol_flows`, `process_network_profiles`,
primary failure fields, ambiguity metadata, and an `augmentations` array that
future rule-based or ML passes can append to without changing the core report
surfaces. The built-in chain already uses that slot for advisory machine hints
such as `unverified_ingest_lineage`, `competing_hypotheses`, and an
`automation_recommendation` item that gives downstream services a conservative
next-action hint. If you later compose external enrich/rerank passes, prefer
stacking them on top of the built-in chain rather than replacing it.

`report.json` remains useful as a richer rendered report surface, but it should
not be treated as the narrow primary machine contract when `summary.json` or
`analysis.json` already fits the integration.

If you want `gewyvern` itself to call an external engine and merge those
augmentations back into its own `analysis.json` and report surfaces, add an
external hook:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --serve --api-socket 127.0.0.1:9100 --json --summary-only --external-engine-bin /Users/Shared/chroot/dev/etragon/target/debug/etragon
```

To route through a Python-backed worker path instead of the engine's default
Rust pass:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --serve --api-socket 127.0.0.1:9100 --json --summary-only --external-engine-bin /Users/Shared/chroot/dev/etragon/target/debug/etragon --external-engine-worker /Users/Shared/chroot/dev/etragon/scripts/python_baseline_worker.py
```

`etragon` is a sibling implementation of that protocol, not a build-time
dependency of `gewyvern`. The generic `--external-engine-*` names are the
supported public surface.

For target-specific routes, discover names from `/v1/latest/targets` and prefer
the returned `target_refs[].path_segment` value when building URLs. The API
accepts percent-encoded path segments and reports its path-segment contract in
`/v1/capabilities`.

Roundtrip demo:

```bash
bash scripts/socket_roundtrip_demo.sh /tmp/gewyvern.sock udp /tmp/gewyvern-out.json unix
bash scripts/socket_roundtrip_demo.sh 127.0.0.1:9000 udp /tmp/gewyvern-out.json tcp
```

Generic external-engine roundtrip demo:

```bash
bash scripts/external_engine_roundtrip_demo.sh 127.0.0.1:9900 127.0.0.1:9910 udp /tmp/gewyvern-analysis.json /tmp/external-engine-augmentations.json
```

By default this looks for a sibling `/../etragon` repo and runs:

```bash
cargo run -- analyze-url
```

inside that engine root. To point it at a different implementation, set:

```bash
ENGINE_ROOT=/path/to/external-engine
EXTERNAL_ENGINE_CMD='cargo run -- analyze-url'
```

If you want the bridge to consume a target-specific route instead of the latest
top-level analysis snapshot, pass the already URL-safe target path segment as a
sixth argument:

```bash
bash scripts/external_engine_roundtrip_demo.sh 127.0.0.1:9900 127.0.0.1:9910 udp /tmp/gewyvern-analysis.json /tmp/external-engine-augmentations.json socket_session
```

Those scripts exercise the full bridge:

1. start `gewyvern` in `--serve` mode with the read-only API enabled
2. ingest a demo socket session
3. let an external engine pull `/v1/latest/analysis.json` directly with `analyze-url`
4. save both the raw analysis snapshot and the external augmentation output

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

- [docs/index.md](/Users/Shared/chroot/dev/gewyvern/docs/index.md)
- [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
- [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
- [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)

## Near-Term Direction

The next meaningful step is not only “more protocol branches”.
It is continuing to make the DSL and IR more explicit, so protocol behavior is
described as program-network-module structure rather than as a pile of
protocol-specific special cases, while steadily closing the remaining gaps in
the active `1.4.x` line and beyond. The concrete release path is tracked in
[ROADMAP.md](/Users/Shared/chroot/dev/gewyvern/ROADMAP.md).
