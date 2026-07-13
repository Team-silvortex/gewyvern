# gewyvern v1.0.0

<p align="center">
  <img src="docs/assets/branding/gewyvern-logo-v1.svg" alt="gewyvern logo" width="240">
</p>

Protocol-aware local network debugging runtime driven by eBPF fragments,
`gewylang` packages, and deterministic runtime surfaces.

`gewyvern` is not trying to be a long-running observability platform. The
current shape is a single-host, window-bounded debugger/runtime that:

- composes eBPF fragments into an attach plan
- ingests structured kernel facts
- reconstructs transport flows and higher-level program flows
- derives deterministic reason chains
- exports replayable JSON, HTML reports, and API-backed runtime surfaces

The repository now also carries the nearby stack pieces that make the debugger
usable as a system:

- `gewyvern`: Linux/eBPF-oriented runtime, compiler front end, protocol
  registry, persistence, config, logging, certificates, and runtime API
- `etragon`: local learning/diagnosis sidecar that can enrich gewyvern output
- `leserpent`: cross-platform control-plane shell for coordinating instances

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

# Validate startup, malformed input recovery, logging, shutdown, and cleanup
bash scripts/validation/runtime_lifecycle_validation.sh

# Start a bounded runtime API surface for local inspection
cargo run -- --protocol http --entry request --serve \
  --tcp-socket 127.0.0.1:9000 \
  --api-socket 127.0.0.1:9100 \
  --json --summary-only --max-sessions 1

# Compile a DSL file or package without starting the runtime
cargo run -p gewyc -- dsl/http_request_path.gewy --json
```

`--summary-only --json` is now the fastest operational view: it includes a
`protocol_flows` array and `process_network_profiles` summary that show whether
each matched protocol path is healthy or currently stuck at a missing
transition. `--report-format html` renders the same single-target or full scan
as a visual report. When `--api-socket` is active, the runtime also exposes
machine surfaces such as `/health`, `/v1/runtime/resilience.json`,
`/v1/protocols`, protocol catalog snapshots, certificate state, and latest
session data.

## Status

- project version: `1.0.0`
- stage: first sealed stable line with repeatable release gates, reliability
  hardening, lifecycle discipline, and real Linux physical-machine validation
  on top of the broad protocol catalog built during the earlier minor lines
- transport support: TCP + UDP
- protocol registry coverage: 70 protocol families and 363 package entries
  under `protocols/`
- input modes: demo facts, Unix socket, TCP socket
- Linux probe support: tracepoint, kprobe, tc ingress smoke/probe paths
- replay: deterministic for exported sessions
- DSL shape: pipeline-driven `gewylang` stable subset
- package shape: `gewy.pkg` manifest + `main.gewy` entry + pipeline
  `include(...)` expansion
- package resolution: `gewyc lock` emits a resolved `gewy.lock` snapshot
- runtime lifecycle: validated startup, malformed input recovery, structured
  log evidence, explicit API shutdown, stop behavior, and temp cleanup
- persistence: latest snapshots plus minor-line history artifacts under the
  standard state root
- config and state layout: documented standard paths with env overrides and
  legacy fallback behavior
- security posture: loopback-first local runtime, protected remote API use
  only when explicit remote bind and runtime admin-token requirements are both
  satisfied, plus certificate policy/state surfaces
- workspace shape: `gewyvern` runtime crate, `gewyc` compiler CLI crate,
  `apps/etragon`, and `apps/leserpent`
- protocol registry shape: scanned gewy project packages under `protocols/`

## Current Release Line

`gewyvern` is no longer just a convergence story. The current line is:

- historical validation baseline: `v0.10.0`
- current release line: `v1.0.0`, the first sealed stable line
- current focus: keep the broad protocol catalog behaving like one integrated
  local network debugger with stable startup, stop, logs, recovery,
  persistence, cross-validation, and Linux-host execution paths

The goal is still not “every protocol under the sun”. The `1.0.0` bar is that
`gewyvern` is trustworthy enough to serve as infrastructure for process-level
network debugging: stable CLI/runtime behavior, stable DSL/compiler boundaries,
reliable HTML/JSON/API reporting, predictable operational performance, and
clean lifecycle behavior with no mystery leftovers.

Primary release-line shelves:

- [ROADMAP.md](ROADMAP.md)
- [docs/history/v1.0.0-release-notes.md](docs/history/v1.0.0-release-notes.md)
- [docs/v0.14-posture.md](docs/v0.14-posture.md)
- [docs/history/index.md](docs/history/index.md)
- [docs/history/v1.0.0.md](docs/history/v1.0.0.md)
- [docs/history/v0.19.x.md](docs/history/v0.19.x.md)
- [docs/machine-contract.md](docs/machine-contract.md)
- [docs/security-posture.md](docs/security-posture.md)
- [docs/service-behavior.md](docs/service-behavior.md)
- [docs/book/reference-runtime-layout.md](docs/book/reference-runtime-layout.md)
- [docs/book/reference-runtime-config.md](docs/book/reference-runtime-config.md)
- [docs/book/reference-runtime-events.md](docs/book/reference-runtime-events.md)

## Supported Protocol Families

- Web, secure transport, media, and proxying:
  HTTP, HTTPS, TLS, QUIC, HTTP/3, Hysteria 2, RTSP, SIP, SOCKS5
- Name resolution and local discovery:
  DNS, DNS-over-TCP, mDNS, SSDP
- L2/L3 discovery, routing, tunnels, and network control:
  ARP, NDP, ICMP, ICMPv6, DHCP, NTP, BGP, OSPF, GRE, GTP-U, IPsec,
  WireGuard, VXLAN, Geneve, L2TP, PPTP, STUN, CoAP, RADIUS, SNMP
- Data stores, brokers, queues, and cache access:
  Redis, Memcached, PostgreSQL, MySQL, MQTT, AMQP, Kafka, NATS
- Mail, identity, directory, file-transfer, and remote desktop/access:
  SMTP, IMAP, POP3, LDAP, Kerberos, FTP, SSH, SMB, RDP

Most built-in entries model a concrete program-network path such as
request/response, auth/query, relay setup, or publish/ack, rather than only
matching a port number.

For the full operator-facing protocol shelf, use
[docs/book/reference-protocol-volume.md](docs/book/reference-protocol-volume.md)
and
[docs/book/reference-protocol-standard-library.md](docs/book/reference-protocol-standard-library.md).

## Repository Shape

This repository is easier to read as one ecosystem workspace with clear
responsibility boundaries:

- [Cargo.toml](Cargo.toml)
  Root workspace manifest.
- [src](src)
  Runtime, IR, DSL compiler front-end, export/replay, loader, and built-in CLI.
- [src/bin](src/bin)
  Helper binaries such as socket senders used by local/runtime demos.
- [crates/gewyc](crates/gewyc)
  Dedicated `.gewy` compiler CLI surface.
- [apps/etragon](apps/etragon)
  Nearby diagnosis-partner sidecar crate; follows the root `gewyvern` version.
- [apps/leserpent](apps/leserpent)
  Cross-platform control plane application; follows the root `gewyvern` version.
- [dsl](dsl)
  Built-in protocol and debugging DSL files.
- [protocols](protocols)
  Registry-style gewy protocol packages scanned into built-in protocol entries.
- [tests](tests)
  TDD coverage for runtime, fragments, templates, compiler, and Linux smoke.
- [docs](docs)
  System, architecture, DSL, validation, packaging, and release guides.
- [ebpf](ebpf)
  Current hand-written eBPF fragment sources and smoke assets.
- [docker](docker)
  Headless Linux dev/smoke environment support.
- [scripts](scripts)
  Grouped operator helpers:
  `packaging/`, `validation/`, `demos/`, `linux/`, `perf/`, and `history/`.
- [packaging](packaging)
  Native Linux packaging templates for DEB/RPM metadata.

## Documentation Entrypoints

Use the docs in layers:

- [docs/index.md](docs/index.md)
  Durable top-level map for project, runtime, DSL, validation, and packaging.
- [docs/book/index.md](docs/book/index.md)
  Structured reading spine for tutorials, how-to, reference, and explanation.
- [docs/script-entrypoints.md](docs/script-entrypoints.md)
  Goal-based script/operator map.
- [docs/cli-recipes.md](docs/cli-recipes.md)
  Runtime CLI, `gewyc`, socket ingest, API, and demo command shelf.

If you are about to operate or expose a real runtime instance, also open:

- [docs/book/how-to-security-checklist.md](docs/book/how-to-security-checklist.md)

If you only want the project's current core contract surfaces, start with:

- [docs/machine-contract.md](docs/machine-contract.md)
- [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
- [docs/book/reference-runtime-layout.md](docs/book/reference-runtime-layout.md)
- [docs/book/reference-runtime-certificate-policy.md](docs/book/reference-runtime-certificate-policy.md)

If you are orienting around architecture specifically, the shortest useful
order is:

1. [docs/architecture-blueprint.md](docs/architecture-blueprint.md)
2. [docs/system.md](docs/system.md)
3. [docs/architecture.md](docs/architecture.md)
4. [docs/architecture-blueprint-modules.md](docs/architecture-blueprint-modules.md)
5. [docs/module-boundaries.md](docs/module-boundaries.md)

If you are orienting yourself for the first time, the shortest useful order is:

1. [README.md](README.md)
2. [docs/index.md](docs/index.md)
3. [docs/book/tutorial-first-run.md](docs/book/tutorial-first-run.md)
4. [docs/dsl.md](docs/dsl.md)
5. [docs/development.md](docs/development.md)

If you already know the system and only need the right operator or validation
entrypoint, jump to
[docs/script-entrypoints.md](docs/script-entrypoints.md).

## Main Entrypoints

Script naming guide used in this repo:

- `roundtrip`: one narrow end-to-end consumption path
- `smoke`: one lightweight existence or bring-up check
- `validation`: one grouped stability check with explicit expectations
- `summary`: one wrapper that runs several narrower validations in order

Directory split used under [`scripts`](scripts):

- `scripts/packaging/`
  Build, install, packaged validation, and release-gate entrypoints.
- `scripts/validation/`
  Runtime, registry, field, and multi-module stability checks.
- `scripts/demos/`
  Narrow roundtrip demos for sockets, external engines, and training datasets.
- `scripts/linux/`
  Linux eBPF attach/kprobe/tc smoke helpers.
- `scripts/perf/`
  Benchmark convenience wrappers.
- `scripts/history/`
  Historical artifact/render helpers.

If you want the shortest goal-based script map instead of the full shelf below,
use [docs/script-entrypoints.md](docs/script-entrypoints.md).

Core CLI and test entrypoints:

- `cargo run -- ...`
  Start the main `gewyvern` runtime CLI.
- `cargo run -p gewyc -- ...`
  Compile or inspect `.gewy` files without starting a runtime session.
- `cargo run -p gewyc -- init my_app`
  Scaffold a minimal gewy package.
- `cargo run -p gewyc -- lock my_app`
  Resolve a gewy package manifest into a `gewy.lock` snapshot.
- `cargo test --workspace`
  Main regression path for the whole workspace.

Packaging entrypoints:

- `bash scripts/packaging/build_packages.sh --layout-only`
- `bash scripts/packaging/build_packages_in_container.sh --format all`
- `bash scripts/packaging/package_install_smoke.sh`
- `bash scripts/packaging/container_runtime_validation.sh`
- `bash scripts/packaging/container_validation_summary.sh`
- `bash scripts/packaging/release_container_check.sh`
- `bash scripts/packaging/release_gate.sh`

Validation and integration entrypoints:

- `bash scripts/validation/registry_validation.sh`
- `bash scripts/validation/high_frequency_validation.sh`
- `bash scripts/validation/runtime_operator_validation.sh`
- `bash scripts/validation/runtime_lifecycle_validation.sh`
- `bash scripts/validation/pathological_container_validation.sh`
- `bash scripts/validation/three_module_stack_smoke.sh`

Roundtrip demos:

- `bash scripts/demos/socket_roundtrip_demo.sh /tmp/gewyvern.sock udp /tmp/gewyvern-out.json unix`
- `bash scripts/demos/external_engine_roundtrip_demo.sh 127.0.0.1:9900 127.0.0.1:9910 udp /tmp/gewyvern-analysis.json /tmp/external-engine-augmentations.json`
- `bash scripts/demos/training_dataset_roundtrip_demo.sh 127.0.0.1:9910 /tmp/gewyvern-training-roundtrip`

Performance and history helpers:

- `bash scripts/perf/benchmark_summary.sh`
- `bash scripts/history/render_minor_line_ir_snapshot.sh`

## Capability Snapshot

The current line is already useful for:

- bounded standalone debugging through CLI or `--serve`
- scanned protocol-package resolution from `protocols/`
- deterministic JSON, HTML, and runtime API reporting surfaces
- compiler-front-end and IR inspection through `gewyc`
- explicit runtime lifecycle validation for startup, recovery, stop, logs, and
  cleanup
- packaged `deb` and `rpm` validation and release gating
- nearby collaboration with `etragon` and `leserpent`

For the deeper durable shelves behind those capabilities, use:

- [docs/architecture-blueprint.md](docs/architecture-blueprint.md)
- [docs/system.md](docs/system.md)
- [docs/dsl.md](docs/dsl.md)
- [docs/dsl-syntax.md](docs/dsl-syntax.md)
- [docs/dsl-reference.md](docs/dsl-reference.md)
- [docs/book/reference-protocol-surface.md](docs/book/reference-protocol-surface.md)
- [docs/book/reference-protocol-volume.md](docs/book/reference-protocol-volume.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
- [docs/book/reference-runtime-layout.md](docs/book/reference-runtime-layout.md)
- [docs/service-behavior.md](docs/service-behavior.md)

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
  -> Runtime API / Export JSON / HTML Report
  -> Latest + History Snapshots
  -> Deterministic Replay
```

For the runtime/IR narrative behind this pipeline, use:

- [docs/book/explanation-gewy-to-runtime.md](docs/book/explanation-gewy-to-runtime.md)
- [docs/book/explanation-gewylang-to-ir.md](docs/book/explanation-gewylang-to-ir.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)

## Command Recipes

The detailed command shelf now lives in:

- [docs/cli-recipes.md](docs/cli-recipes.md)
- [docs/script-entrypoints.md](docs/script-entrypoints.md)

If you are learning the CLI for the first time, the best paired reading order
is:

1. [docs/book/tutorial-first-run.md](docs/book/tutorial-first-run.md)
2. [docs/cli-recipes.md](docs/cli-recipes.md)
3. [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)

## Development

The contributor workflow shelf is intentionally separated:

- [docs/development.md](docs/development.md)
- [docs/performance-baselines.md](docs/performance-baselines.md)
- [docs/headless-linux.md](docs/headless-linux.md)

Use these instead of treating the README as the command notebook.

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
- program-flow reconstruction is still intentionally small and conservative
- local Unix/TCP socket live tests are ignored in restricted environments that
  do not allow bind permissions
- `tc egress` is not implemented yet in the real Linux probe path

## Repo Docs

The three main doc front doors are:

- [docs/index.md](docs/index.md)
  Top-level shelf map.
- [docs/book/index.md](docs/book/index.md)
  Reading order if you want the project as a book.
- [docs/script-entrypoints.md](docs/script-entrypoints.md)
  and
  [docs/cli-recipes.md](docs/cli-recipes.md)
  Fast operator lookup for scripts and commands.

Then branch by topic:

- architecture:
  [docs/architecture-blueprint.md](docs/architecture-blueprint.md),
  [docs/system.md](docs/system.md)
- `gewylang`:
  [docs/dsl.md](docs/dsl.md),
  [docs/dsl-syntax.md](docs/dsl-syntax.md),
  [docs/dsl-reference.md](docs/dsl-reference.md)
- contributor workflow:
  [docs/development.md](docs/development.md)

## Near-Term Direction

The next meaningful step is not only “more protocol branches”.
It is making the broad protocol shelf feel like one integrated debugger:
protocol packages should lower toward the same IR vocabulary, runtime exits
should stay clean, logs and state should explain what happened, and the local
operator loop should remain predictable as post-`1.0.0` physical-machine and
cross-validation testing expands. The concrete release path is tracked in
[ROADMAP.md](ROADMAP.md).
