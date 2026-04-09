# gewyvern v0.1

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

## Status

- project version: `0.1.0`
- stage: working prototype
- transport support: TCP + UDP
- input modes: demo facts, Unix socket, TCP socket
- Linux probe support: tracepoint, kprobe, tc ingress smoke/probe paths
- replay: deterministic for exported sessions

## What Works In v0.1

- Fragment registry, attach planning, and attach reporting
- TDD-first runtime and rule specs
- Window-bounded sessions with `freeze(end)` and late-arrival cutoff
- Fact ingest gating based on real attach outcomes
- Rejected fact audit trail and aggregated summaries
- Transport flow reconstruction from packet/state/route/lineage facts
- Program flow reconstruction for process-aware network behavior
- Deterministic reason chains for:
  - TCP handshake-oriented sessions
  - UDP datagram-oriented sessions
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

## DSL Files

The repository now includes first-class DSL files that compile into
`TemplateBinding` rather than into eBPF bytecode:

- [dsl/handshake_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/handshake_debug.gewy)
- [dsl/udp_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/udp_debug.gewy)
- [dsl/udp_process_debug.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy)
- [dsl/dns_udp_process.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy)
- [dsl/https_connect_process.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/https_connect_process.gewy)
- [dsl/http_request_path.gewy](/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy)

These DSL files already cover the current built-in protocol/debugging shapes and
can express:

- fragment selection
- window profile selection
- reason profile selection
- program model operation/rules
- fragment parameter bindings
- template-local evidence tier overrides

## Development

This repository is intentionally TDD-driven.

Main commands:

- `cargo tdd`
- `cargo tdd-one <test_name>`
- `cargo tdd-rules`
- `cargo test`

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
```

Inspect binding diagnostics without starting a runtime session:

```bash
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --diagnostics
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy --diagnostics --json
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
- `ProgramModel` is embedded Rust data today, not an external DSL yet
- `ProgramFlow.operation` can now carry template-defined custom ids, but the rule
  surface is still intentionally small
- the intended DSL compile target is `template + fragment params`, not eBPF bytecode
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

The next meaningful step after `v0.1` is not “more protocol branches”.
It is pushing `ProgramModel` from embedded Rust rules toward a more explicit,
protocol-agnostic DSL over fragment/attach/fact IR so the engine can model
network functionality as program behavior rather than just protocol lifecycle.
