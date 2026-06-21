# gewyvern v0.14.0

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

- project version: `0.14.0`
- stage: active `0.14.x` line focused on protocol depth, compiler ergonomics,
  runtime/report stability, and cleaner collaboration surfaces across
  `gewyvern`, `etragon`, and `leserpent`
- transport support: TCP + UDP
- protocol path coverage in DSL: DNS, HTTP, TLS, QUIC, STUN, CoAP, NTP, DHCP,
  WireGuard, mDNS, SSDP, Redis, MQTT, PostgreSQL, MySQL, Memcached, AMQP,
  RADIUS, GTP-U, SMTP, SSH, SOCKS5, SIP, LDAP, SNMP, RTSP, DNS-over-TCP
- input modes: demo facts, Unix socket, TCP socket
- Linux probe support: tracepoint, kprobe, tc ingress smoke/probe paths
- replay: deterministic for exported sessions
- DSL shape: pipeline-driven `gewylang` stable subset
- package shape: `gewy.pkg` manifest + `main.gewy` entry + pipeline
  `include(...)` expansion
- package resolution: `gewyc lock` emits a resolved `gewy.lock` snapshot
- workspace shape: `gewyvern` runtime crate + `gewyc` compiler CLI crate
- protocol registry shape: scanned gewy project packages under `protocols/`

## Current Release Line

`gewyvern` is no longer just a convergence story. The current line is:

- historical validation baseline: `v0.10.0`
- current release line: `v0.14.0`
- current focus: deepen protocol quality, keep runtime/report/compiler behavior
  predictable, and make cross-project collaboration (`gewyvern` + `etragon` +
  `leserpent`) more deliberate without bloating the standalone debugger core
- next likely work line: `v0.15.x`, unless a later architectural break justifies
  a deliberately chosen `v2.0`

The goal is still not “every protocol under the sun”. The `0.14.x` bar is that
`gewyvern` is trustworthy enough to serve as infra for process-level network
debugging: stable CLI/runtime behavior, stable DSL/compiler boundaries,
reliable HTML/JSON reporting, and predictable operational performance.

Primary release-line shelves:

- [ROADMAP.md](/Users/Shared/chroot/dev/gewyvern/ROADMAP.md)
- [docs/v0.14-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.14-posture.md)
- [docs/history/index.md](/Users/Shared/chroot/dev/gewyvern/docs/history/index.md)
- [docs/machine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/machine-contract.md)
- [docs/security-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/security-posture.md)
- [docs/service-behavior.md](/Users/Shared/chroot/dev/gewyvern/docs/service-behavior.md)

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

This repository is easier to read as one ecosystem workspace with clear
responsibility boundaries:

- [Cargo.toml](/Users/Shared/chroot/dev/gewyvern/Cargo.toml)
  Root workspace manifest.
- [src](/Users/Shared/chroot/dev/gewyvern/src)
  Runtime, IR, DSL compiler front-end, export/replay, loader, and built-in CLI.
- [src/bin](/Users/Shared/chroot/dev/gewyvern/src/bin)
  Helper binaries such as socket senders used by local/runtime demos.
- [crates/gewyc](/Users/Shared/chroot/dev/gewyvern/crates/gewyc)
  Dedicated `.gewy` compiler CLI surface.
- [apps/etragon](/Users/Shared/chroot/dev/gewyvern/apps/etragon)
  Nearby diagnosis-partner sidecar crate, version `0.1.0`.
- [apps/leserpent](/Users/Shared/chroot/dev/gewyvern/apps/leserpent)
  Cross-platform control plane application, version `0.1.9`.
- [dsl](/Users/Shared/chroot/dev/gewyvern/dsl)
  Built-in protocol and debugging DSL files.
- [protocols](/Users/Shared/chroot/dev/gewyvern/protocols)
  Registry-style gewy protocol packages scanned into built-in protocol entries.
- [tests](/Users/Shared/chroot/dev/gewyvern/tests)
  TDD coverage for runtime, fragments, templates, compiler, and Linux smoke.
- [docs](/Users/Shared/chroot/dev/gewyvern/docs)
  System, architecture, DSL, validation, packaging, and release guides.
- [ebpf](/Users/Shared/chroot/dev/gewyvern/ebpf)
  Current hand-written eBPF fragment sources and smoke assets.
- [docker](/Users/Shared/chroot/dev/gewyvern/docker)
  Headless Linux dev/smoke environment support.
- [scripts](/Users/Shared/chroot/dev/gewyvern/scripts)
  Grouped operator helpers:
  `packaging/`, `validation/`, `demos/`, `linux/`, `perf/`, and `history/`.
- [packaging](/Users/Shared/chroot/dev/gewyvern/packaging)
  Native Linux packaging templates for DEB/RPM metadata.

## Documentation Entrypoints

Use the docs in layers:

- [docs/index.md](/Users/Shared/chroot/dev/gewyvern/docs/index.md)
  Durable top-level map for project, runtime, DSL, validation, and packaging.
- [docs/book/index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/index.md)
  Structured reading spine for tutorials, how-to, reference, and explanation.
- [docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md)
  Goal-based script/operator map.
- [docs/cli-recipes.md](/Users/Shared/chroot/dev/gewyvern/docs/cli-recipes.md)
  Runtime CLI, `gewyc`, socket ingest, API, and demo command shelf.

If you are about to operate or expose a real runtime instance, also open:

- [docs/book/how-to-security-checklist.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-security-checklist.md)

If you only want the project's current core contract surfaces, start with:

- [docs/machine-contract.md](/Users/Shared/chroot/dev/gewyvern/docs/machine-contract.md)
- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

If you are orienting around architecture specifically, the shortest useful
order is:

1. [docs/architecture-blueprint.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint.md)
2. [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
3. [docs/architecture.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture.md)
4. [docs/architecture-blueprint-modules.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint-modules.md)
5. [docs/module-boundaries.md](/Users/Shared/chroot/dev/gewyvern/docs/module-boundaries.md)

If you are orienting yourself for the first time, the shortest useful order is:

1. [README.md](/Users/Shared/chroot/dev/gewyvern/README.md)
2. [docs/index.md](/Users/Shared/chroot/dev/gewyvern/docs/index.md)
3. [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
4. [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
5. [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)

If you already know the system and only need the right operator or validation
entrypoint, jump to
[docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md).

## Main Entrypoints

Script naming guide used in this repo:

- `roundtrip`: one narrow end-to-end consumption path
- `smoke`: one lightweight existence or bring-up check
- `validation`: one grouped stability check with explicit expectations
- `summary`: one wrapper that runs several narrower validations in order

Directory split used under [`scripts`](/Users/Shared/chroot/dev/gewyvern/scripts):

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
use [docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md).

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

- `bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/build_packages.sh --layout-only`
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/build_packages_in_container.sh --format all`
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/package_install_smoke.sh`
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_runtime_validation.sh`
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/container_validation_summary.sh`
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_container_check.sh`
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/packaging/release_gate.sh`

Validation and integration entrypoints:

- `bash /Users/Shared/chroot/dev/gewyvern/scripts/validation/registry_validation.sh`
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/validation/high_frequency_validation.sh`
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/validation/runtime_operator_validation.sh`
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/validation/three_module_stack_smoke.sh`

Roundtrip demos:

- `bash /Users/Shared/chroot/dev/gewyvern/scripts/demos/socket_roundtrip_demo.sh /tmp/gewyvern.sock udp /tmp/gewyvern-out.json unix`
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/demos/external_engine_roundtrip_demo.sh 127.0.0.1:9900 127.0.0.1:9910 udp /tmp/gewyvern-analysis.json /tmp/external-engine-augmentations.json`
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/demos/training_dataset_roundtrip_demo.sh 127.0.0.1:9910 /tmp/gewyvern-training-roundtrip`

Performance and history helpers:

- `bash /Users/Shared/chroot/dev/gewyvern/scripts/perf/benchmark_summary.sh`
- `bash /Users/Shared/chroot/dev/gewyvern/scripts/history/render_minor_line_ir_snapshot.sh`

## Capability Snapshot

The current line is already useful for:

- bounded standalone debugging through CLI or `--serve`
- scanned protocol-package resolution from `protocols/`
- deterministic JSON and HTML reporting surfaces
- compiler-front-end and IR inspection through `gewyc`
- packaged `deb` and `rpm` validation and release gating
- nearby collaboration with `etragon` and `leserpent`

For the deeper durable shelves behind those capabilities, use:

- [docs/architecture-blueprint.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint.md)
- [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)
- [docs/dsl-syntax.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-syntax.md)
- [docs/dsl-reference.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-reference.md)
- [docs/book/reference-protocol-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-protocol-surface.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
- [docs/service-behavior.md](/Users/Shared/chroot/dev/gewyvern/docs/service-behavior.md)

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

For the runtime/IR narrative behind this pipeline, use:

- [docs/book/explanation-gewy-to-runtime.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewy-to-runtime.md)
- [docs/book/explanation-gewylang-to-ir.md](/Users/Shared/chroot/dev/gewyvern/docs/book/explanation-gewylang-to-ir.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)

## Command Recipes

The detailed command shelf now lives in:

- [docs/cli-recipes.md](/Users/Shared/chroot/dev/gewyvern/docs/cli-recipes.md)
- [docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md)

If you are learning the CLI for the first time, the best paired reading order
is:

1. [docs/book/tutorial-first-run.md](/Users/Shared/chroot/dev/gewyvern/docs/book/tutorial-first-run.md)
2. [docs/cli-recipes.md](/Users/Shared/chroot/dev/gewyvern/docs/cli-recipes.md)
3. [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)

## Development

The contributor workflow shelf is intentionally separated:

- [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)
- [docs/performance-baselines.md](/Users/Shared/chroot/dev/gewyvern/docs/performance-baselines.md)
- [docs/headless-linux.md](/Users/Shared/chroot/dev/gewyvern/docs/headless-linux.md)

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

- [docs/index.md](/Users/Shared/chroot/dev/gewyvern/docs/index.md)
  Top-level shelf map.
- [docs/book/index.md](/Users/Shared/chroot/dev/gewyvern/docs/book/index.md)
  Reading order if you want the project as a book.
- [docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md)
  and
  [docs/cli-recipes.md](/Users/Shared/chroot/dev/gewyvern/docs/cli-recipes.md)
  Fast operator lookup for scripts and commands.

Then branch by topic:

- architecture:
  [docs/architecture-blueprint.md](/Users/Shared/chroot/dev/gewyvern/docs/architecture-blueprint.md),
  [docs/system.md](/Users/Shared/chroot/dev/gewyvern/docs/system.md)
- `gewylang`:
  [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md),
  [docs/dsl-syntax.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-syntax.md),
  [docs/dsl-reference.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl-reference.md)
- contributor workflow:
  [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)

## Near-Term Direction

The next meaningful step is not only “more protocol branches”.
It is continuing to make the DSL and IR more explicit, so protocol behavior is
described as program-network-module structure rather than as a pile of
protocol-specific special cases, while steadily closing the remaining gaps in
the active `0.14.x` line and beyond. The concrete release path is tracked in
[ROADMAP.md](/Users/Shared/chroot/dev/gewyvern/ROADMAP.md).
