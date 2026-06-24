# CLI Recipes

This page keeps the practical command shelf for the active `0.17.x` line.

Use it when you already understand the project shape and just want known-good
commands for the runtime CLI, `gewyc`, socket ingest, and local integration
paths.

For the broader project map, start with:

- [README.md](/Users/Shared/chroot/dev/gewyvern/README.md)
- [docs/index.md](/Users/Shared/chroot/dev/gewyvern/docs/index.md)
- [docs/script-entrypoints.md](/Users/Shared/chroot/dev/gewyvern/docs/script-entrypoints.md)
- [docs/monorepo-stack.md](/Users/Shared/chroot/dev/gewyvern/docs/monorepo-stack.md)

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
cargo run -- --scan-all --summary-only --report-format html --out /tmp/gewyvern-scan-report.html
```

Use these when you want the shortest runtime proof that:

- protocol registration still resolves
- one built-in protocol path still works
- the current sweep path still renders machine and HTML outputs

## DSL-Focused Runtime Commands

```bash
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/quic_client_initial_path.gewy --json --summary-only
cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy --json --summary-only
```

Use these when the question is about one concrete `.gewy` path rather than the
whole built-in registry.

## `gewyc` Commands

```bash
cargo run -p gewyc -- /Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy --json
cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy --focus binding
cargo run -p gewyc -- diagnostics /Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy --json
cargo run -p gewyc -- findings /Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy --json
cargo run -p gewyc -- stages /Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy --json
cargo run -p gewyc -- envelope /Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy --json
```

Use these when you want compiler-facing visibility without starting a runtime
session.

Related references:

- [docs/gewyc-json.md](/Users/Shared/chroot/dev/gewyvern/docs/gewyc-json.md)
- [docs/book/reference-ir-lowering.md](/Users/Shared/chroot/dev/gewyvern/docs/book/reference-ir-lowering.md)
- [docs/dsl.md](/Users/Shared/chroot/dev/gewyvern/docs/dsl.md)

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

- [docs/service-behavior.md](/Users/Shared/chroot/dev/gewyvern/docs/service-behavior.md)
- [docs/book/how-to-security-checklist.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-security-checklist.md)
- [docs/book/how-to-validate-runtime-surface.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-validate-runtime-surface.md)

## Read-Only API Endpoints

High-value endpoints during `--serve`:

- `/health`
- `/v1/capabilities`
- `/v1/latest/meta`
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
- `/v1/latest/targets/<name>/export.json`
- `/v1/latest/targets/<name>/report.json`
- `/v1/latest/targets/<name>/report.html`

When another service needs a narrow machine-facing surface, prefer:

- `summary.json` for compact status
- `analysis.json` for protocol flows, process profiles, failure semantics, and
  append-only augmentations

## Roundtrip Demos

Socket roundtrip:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/demos/socket_roundtrip_demo.sh /tmp/gewyvern.sock udp /tmp/gewyvern-out.json unix
bash /Users/Shared/chroot/dev/gewyvern/scripts/demos/socket_roundtrip_demo.sh 127.0.0.1:9000 udp /tmp/gewyvern-out.json tcp
```

External-engine bridge:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/demos/external_engine_roundtrip_demo.sh 127.0.0.1:9900 127.0.0.1:9910 udp /tmp/gewyvern-analysis.json /tmp/external-engine-augmentations.json
```

Training dataset roundtrip:

```bash
bash /Users/Shared/chroot/dev/gewyvern/scripts/demos/training_dataset_roundtrip_demo.sh 127.0.0.1:9910 /tmp/gewyvern-training-roundtrip
```

These are the thinnest end-to-end consumer checks when you do not want a full
validation shelf.

## Development Commands

```bash
cargo test --workspace
cargo tdd
cargo tdd-one <test_name>
cargo tdd-rules
bash /Users/Shared/chroot/dev/gewyvern/scripts/perf/benchmark_summary.sh
```

For contributor workflow and layout guidance, use:

- [docs/development.md](/Users/Shared/chroot/dev/gewyvern/docs/development.md)
- [docs/performance-baselines.md](/Users/Shared/chroot/dev/gewyvern/docs/performance-baselines.md)

## Linux Probe Work

If the change touches real eBPF attach/runtime behavior, use:

- [docs/headless-linux.md](/Users/Shared/chroot/dev/gewyvern/docs/headless-linux.md)
- [scripts/linux/linux_attach_smoke.sh](/Users/Shared/chroot/dev/gewyvern/scripts/linux/linux_attach_smoke.sh)
- [scripts/linux/linux_kprobe_smoke.sh](/Users/Shared/chroot/dev/gewyvern/scripts/linux/linux_kprobe_smoke.sh)
- [scripts/linux/linux_tc_smoke.sh](/Users/Shared/chroot/dev/gewyvern/scripts/linux/linux_tc_smoke.sh)
