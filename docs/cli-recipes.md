# CLI Recipes

This page keeps the practical command shelf for the active `0.19.x` line.

Use it when you already understand the project shape and just want known-good
commands for the runtime CLI, `gewyc`, socket ingest, and local integration
paths.

For the broader project map, start with:

- [README.md](README.md)
- [docs/index.md](docs/index.md)
- [docs/script-entrypoints.md](docs/script-entrypoints.md)
- [docs/monorepo-stack.md](docs/monorepo-stack.md)

## Monorepo Stack Commands

Use these when the question is about the whole local stack rather than only the
runtime crate:

```bash
# Rust workspace
cargo test --workspace

# etragon sidecar
cargo run -p etragon -- --help

# leserpent frontend + backend
cd apps/leserpent && npm run check:frontend
dotnet build apps/leserpent/src/Leserpent/Leserpent.csproj
```

## Fastest Runtime Commands

```bash
cargo run -- --list-protocols
cargo run -- --list-entries quic
cargo run -- --protocol mysql --entry session --json --summary-only
cargo run -- --scan-all --json --summary-only
cargo run -- --scan-all --debug-session --json
cargo run -- --scan-all --summary-only --report-format html --out /tmp/gewyvern-scan-report.html
cargo run --quiet --bin gewyvern_validate -- debugger-cross
```

Use these when you want the shortest runtime proof that:

- protocol registration still resolves
- one built-in protocol path still works
- the current sweep path still renders debug-session, machine, and HTML outputs
- debug-session includes a conservative `debugger_route` for the next safe
  surface to open
- the debugger cross-check still agrees across runtime, console, and compiler
  envelope surfaces

## DSL-Focused Runtime Commands

```bash
cargo run -- --dsl dsl/http_request_path.gewy --json --summary-only
cargo run -- --dsl dsl/quic_client_initial_path.gewy --json --summary-only
cargo run -- --dsl dsl/dns_udp_process.gewy --json --summary-only
```

Use these when the question is about one concrete `.gewy` path rather than the
whole built-in registry.

## `gewyc` Commands

```bash
cargo run -p gewyc -- dsl/http_request_path.gewy --json
cargo run -p gewyc -- explain dsl/http_request_path.gewy --focus binding
cargo run -p gewyc -- diagnostics dsl/http_request_path.gewy --json
cargo run -p gewyc -- findings dsl/http_request_path.gewy --json
cargo run -p gewyc -- stages dsl/http_request_path.gewy --json
cargo run -p gewyc -- envelope dsl/http_request_path.gewy --json
```

Use these when you want compiler-facing visibility without starting a runtime
session.

Related references:

- [docs/gewyc-json.md](docs/gewyc-json.md)
- [docs/book/reference-ir-lowering.md](docs/book/reference-ir-lowering.md)
- [docs/dsl.md](docs/dsl.md)

## Socket Ingest Recipes

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

Live `--serve` plus API:

```bash
cargo run -- --scan-all --tcp-socket 127.0.0.1:9000 --serve --api-socket 127.0.0.1:9100 --json --summary-only
```

Human-facing ingest mode examples:

```bash
cargo run -- --protocol mysql --entry session --tcp-socket 127.0.0.1:9000 --ingest-mode local-advisory --json
cargo run -- --protocol mysql --entry session --tcp-socket 0.0.0.0:9000 --ingest-mode remote-advisory --json
```

For the behavioral contract behind these commands, see:

- [docs/service-behavior.md](docs/service-behavior.md)
- [docs/book/how-to-security-checklist.md](docs/book/how-to-security-checklist.md)
- [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)

## Read-Only API Endpoints

High-value endpoints during `--serve`:

- `/health`
- `/v1/capabilities`
- `/v1/latest/meta`
- `/v1/latest/debug-session.json`
- `/v1/latest/targets`
- `/v1/latest/summary.json`
- `/v1/latest/findings.json`
- `/v1/latest/analysis.json`
- `/v1/latest/export.json`
- `/v1/latest/report.json`
- `/v1/latest/report.html`

Target-scoped variants also exist under:

- `/v1/latest/targets/<name>/summary.json`
- `/v1/latest/targets/<name>/findings.json`
- `/v1/latest/targets/<name>/analysis.json`
- `/v1/latest/targets/<name>/debug-session.json`
- `/v1/latest/targets/<name>/protocol-reading.json`
- `/v1/latest/targets/<name>/export.json`
- `/v1/latest/targets/<name>/report.json`
- `/v1/latest/targets/<name>/report.html`

When another service needs a narrow machine-facing surface, prefer:

- `summary.json` for compact status
- `analysis.json` for protocol flows, process profiles, failure semantics, and
  append-only augmentations
- `debug-session.json` for recommended focus, failure spine, debugger posture,
  and next-step links

## Roundtrip Demos

Socket roundtrip:

```bash
cargo run --quiet --bin gewyvern_validate -- socket-roundtrip --socket-target /tmp/gewyvern.sock --template udp --output /tmp/gewyvern-out.json --socket-kind unix
cargo run --quiet --bin gewyvern_validate -- socket-roundtrip --socket-target 127.0.0.1:9000 --template udp --output /tmp/gewyvern-out.json --socket-kind tcp
```

External-engine bridge:

```bash
cargo run --quiet --bin gewyvern_validate -- external-engine-roundtrip --ingest-addr 127.0.0.1:9900 --api-addr 127.0.0.1:9910 --template udp --analysis-out /tmp/gewyvern-analysis.json --engine-out /tmp/external-engine-augmentations.json
```

Training dataset roundtrip:

```bash
cargo run --quiet --bin gewyvern_validate -- training-roundtrip --api-addr 127.0.0.1:9910 --out-dir /tmp/gewyvern-training-roundtrip
```

These are the thinnest end-to-end consumer checks when you do not want a full
validation shelf.

## Development Commands

```bash
cargo test --workspace
cargo tdd
cargo tdd-one <test_name>
cargo tdd-rules
bash scripts/perf/benchmark_summary.sh
```

For contributor workflow and layout guidance, use:

- [docs/development.md](docs/development.md)
- [docs/performance-baselines.md](docs/performance-baselines.md)

## Linux Probe Work

If the change touches real eBPF attach/runtime behavior, use:

- [docs/headless-linux.md](docs/headless-linux.md)
- [scripts/linux/linux_attach_smoke.sh](scripts/linux/linux_attach_smoke.sh)
- [scripts/linux/linux_kprobe_smoke.sh](scripts/linux/linux_kprobe_smoke.sh)
- [scripts/linux/linux_tc_smoke.sh](scripts/linux/linux_tc_smoke.sh)

Run these with root/BPF attach privileges, for example through `sudo`, and pass
the default-route interface to the TC smoke. An unprivileged run may fail during
libbpf loading with `Operation not permitted`, which is an environment
permission failure rather than a protocol diagnosis failure.
