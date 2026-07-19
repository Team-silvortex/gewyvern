# CLI Recipes

This page keeps the practical command shelf for the active `1.4.6` line.

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

# Locked, warning-free Rust quality gate across libraries, bins, examples, and tests
cargo quality

# etragon sidecar
cargo run -p etragon -- --help

# leserpent frontend + backend
cd apps/leserpent && npm run check:frontend
dotnet restore apps/leserpent/leserpent.slnx --locked-mode
dotnet build apps/leserpent/leserpent.slnx --no-restore
```

`cargo quality` is the canonical Rust lint gate. It fails on any Clippy warning
and covers the complete locked workspace with every Cargo target enabled. If a
fresh rustup toolchain does not include Clippy yet, install the official
component once with `rustup component add clippy`.

## Project Status

Use the native status tensor instead of maintaining a separate progress table:

```bash
cargo run --bin gewyvern_status -- summary
cargo run --bin gewyvern_status -- weakest
cargo run --bin gewyvern_status -- standalone
cargo run --bin gewyvern_status -- developing --architecture leserpent-2
cargo run --bin gewyvern_status -- validate
```

Use `--architecture`, `--module`, `--feature`, `--lifecycle`, and `--maturity`
to slice the tensor. Add `--json` for automation or model context.

## Security Checks

Use these when you want the shortest repeatable security shelf for the current
`1.4.6` line:

```bash
cargo audit
dotnet restore apps/leserpent/leserpent.slnx --locked-mode
dotnet list apps/leserpent/leserpent.slnx package --vulnerable --include-transitive
cd apps/leserpent && npm audit --json
dotnet test apps/leserpent/leserpent.slnx --no-restore
cargo test -p etragon tests::daemon::request_limits::daemon_request_reader_rejects_duplicate_content_length_headers --bin etragon -- --exact --nocapture
cargo test -p etragon tests::daemon::request_limits::daemon_handler_returns_400_for_invalid_request_headers --bin etragon -- --exact --nocapture
cargo test -p etragon tests::daemon::routes::daemon_remote_token_checks_trim_and_match_headers_case_insensitively --bin etragon -- --exact --nocapture
```

This shelf is the practical proof that:

- dependency advisories still come back clean across Rust, .NET, and frontend
  packages
- `leserpent` still enforces remote token and loopback mutate-intent
  boundaries in the real middleware path
- `etragon` still rejects duplicate `Content-Length` and duplicate
  admin-token ambiguity instead of accepting an unsafe request shape

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
  surface to open, plus local `command` hints for the next CLI move
- the debugger cross-check still agrees across runtime, console, and compiler
  envelope surfaces

## Validation JSON Recipes

Use these when the caller is CI, a release bot, or a local wrapper that should
consume one final machine-readable result instead of scraping text logs.

List and help:

```bash
cargo run --quiet --bin gewyvern_validate -- --json list
cargo run --quiet --bin gewyvern_validate -- --json help
```

Fastest release-gate result:

```bash
cargo run --quiet --bin gewyvern_validate -- --json release-gate
cargo run --quiet --bin gewyvern_validate -- --json --json-out /tmp/gewyvern-release-gate.json release-gate
```

Narrow the gate without leaving JSON mode:

```bash
cargo run --quiet --bin gewyvern_validate -- --json release-gate --skip-build
cargo run --quiet --bin gewyvern_validate -- --json release-gate --skip-stack
cargo run --quiet --bin gewyvern_validate -- --json release-gate --skip-debugger-cross
cargo run --quiet --bin gewyvern_validate -- --json release-gate --skip-pathology
cargo run --quiet --bin gewyvern_validate -- --json release-gate --remote-host-validation
```

Remote Linux host evidence as one structured object:

```bash
cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation
```

Practical Linux target-lab evidence as one structured object:

```bash
sudo cargo run --quiet --bin gewyvern_validate -- --json juice-shop-container-validation
sudo cargo run --quiet --bin gewyvern_validate -- --json ftp-denied-container-validation
sudo cargo run --quiet --bin gewyvern_validate -- --json ldap-bind-denied-container-validation
```

Current stable top-level fields:

- `schema_version`
- `ok`
- `command`
- `name`
- `checks`
- `evidence_dir`
- `extra`

Current version gate:

- `schema_version = 1`

High-value `jq` snippets:

```bash
cargo run --quiet --bin gewyvern_validate -- --json release-gate \
  | jq '.schema_version == 1'

cargo run --quiet --bin gewyvern_validate -- --json release-gate \
  | jq '.extra.stages'

cargo run --quiet --bin gewyvern_validate -- --json release-gate --remote-host-validation \
  | jq '.extra.remote.ebpf.status'

cargo run --quiet --bin gewyvern_validate -- --json release-gate --remote-host-validation \
  | jq '{gate_posture: .extra.gate_posture, ship_signal: .extra.ship_signal, next_step: .extra.next_step}'

cargo run --quiet --bin gewyvern_validate -- --json release-gate --remote-host-validation \
  | jq '.extra.remote.slowest_phase_entries'

cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation \
  | jq '.extra.preflight.arch == "x86_64"'

cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation \
  | jq '.extra.phase_timings.remote_package_build'

cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation \
  | jq '.extra.total_seconds'

cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation \
  | jq '.extra.package_build_timings'

cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation \
  | jq '.extra.package_smoke_timings'

cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation \
  | jq '.extra.runtime_smoke_timings'

cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation \
  | jq '.extra.ebpf.default_route_device'

cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation \
  | jq '.extra.budget_warnings // []'

cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation \
  | jq '{posture: .extra.validation_posture, signal: .extra.release_gate_signal, linux_proof_complete: .extra.linux_proof_complete, requires_followup: .extra.requires_followup, next_step: .extra.next_step}'

cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation \
  | jq '.extra.recent_ebpf_trend, .extra.remote_ebpf_status_counts'

cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation \
  | jq '.extra.remote_ebpf_matrix'

cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation \
  | jq '.extra.remote_ebpf_history_integrity'
```

Recent remote eBPF history from the local evidence shelf:

```bash
tail -n 5 target/validation/remote-linux-host-validation/remote-ebpf-history.jsonl
jq '.ebpf.status, .ebpf.reason, .total_seconds' target/validation/remote-linux-host-validation/remote-ebpf-latest.json
cat target/validation/remote-linux-host-validation/remote-ebpf-recent.txt
jq '.integrity, .status_counts, .reason_counts, .matrix' target/validation/remote-linux-host-validation/remote-ebpf-status-summary.json
```

Failure-mode example:

```bash
cargo run --quiet --bin gewyvern_validate -- --json linux-tc-smoke --dev eth0
```

`linux-tc-smoke` fails closed if the interface already owns a `clsact` qdisc.
It never clears pre-existing traffic-control state; use a dedicated validation
interface when the host already runs TC or eBPF networking policy.

If a pipeline wants both stdout and a saved artifact, place the global output
path before the command:

```bash
cargo run --quiet --bin gewyvern_validate -- --json --json-out /tmp/gewyvern-remote.json remote-linux-host-validation
```

That failure shape carries:

- `failure_class`
- `failure_code`
- `message`
- `next_steps`

Current high-value `failure_code` values:

- `invalid_cli_input`
- `docker_unreachable`
- `missing_package_artifact`
- `validation_timeout`
- `remote_workspace_retained`
- `remote_host_not_linux`
- `remote_host_wrong_arch`
- `remote_admin_credentials_incomplete`
- `linux_ebpf_privilege_required`
- `missing_sshpass`
- `missing_system_command`

Use `extra` fields instead of scraping text summaries such as
`remote-ebpf:` or `slowest-phases:` whenever a wrapper can consume JSON.

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

Protected local-first API patterns:

```bash
# local-only API, no token required for loopback callers
cargo run -- --scan-all \
  --tcp-socket 127.0.0.1:9000 \
  --serve \
  --api-socket 127.0.0.1:9100 \
  --json --summary-only

# remote API, explicit exposure plus runtime admin token
GEWY_API_ADMIN_TOKEN='replace-me' \
cargo run -- --scan-all \
  --tcp-socket 127.0.0.1:9000 \
  --serve \
  --api-socket 0.0.0.0:9100 \
  --allow-remote-api \
  --json --summary-only

# equivalent remote API launch with CLI token injection
cargo run -- --scan-all \
  --tcp-socket 127.0.0.1:9000 \
  --serve \
  --api-socket 0.0.0.0:9100 \
  --allow-remote-api \
  --api-admin-token replace-me \
  --json --summary-only
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

When the API stays on loopback, ordinary local reads need no extra header.

For explicit remote API exposure, callers must send:

- `X-Gewyvern-Admin-Token: <token>`

Example:

```bash
curl -H 'X-Gewyvern-Admin-Token: replace-me' \
  http://127.0.0.1:9100/v1/latest/summary.json
```

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
- `cargo run --quiet --bin gewyvern_validate -- linux-attach-smoke`
- `cargo run --quiet --bin gewyvern_validate -- linux-kprobe-smoke`
- `cargo run --quiet --bin gewyvern_validate -- linux-tc-smoke --dev <default-route-device>`
- `sudo cargo run --quiet --bin gewyvern_validate -- juice-shop-container-validation`

Run these with root/BPF attach privileges, for example through `sudo`, and pass
the default-route interface to the TC smoke. An unprivileged run may fail during
libbpf loading with `Operation not permitted`, which is an environment
permission failure rather than a protocol diagnosis failure.

Use `juice-shop-container-validation` when you want one practical Linux target
lab that preserves suspicious HTTP evidence and then immediately proves the
same host can still execute native attach, kprobe, and tc checks. Treat it as a
Linux/BPF confidence command, not as direct web-vulnerability classification.
