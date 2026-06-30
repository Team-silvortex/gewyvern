# Script Entrypoints

This page is the shortest goal-based map for the reorganized `scripts/` tree.

Use it when you know what you want to prove, but do not want to remember which
script shelf currently owns that check.

The naming split used throughout the repository is:

- `roundtrip`: one narrow end-to-end consumer path
- `smoke`: one lightweight bring-up or existence check
- `validation`: one grouped expectation check
- `summary`: one wrapper over narrower validations

## Directory Map

- [`scripts/packaging/`](scripts/packaging)
  Build packages, install them, validate packaged behavior, and run release
  gates.
- [`scripts/validation/`](scripts/validation)
  Validate runtime behavior, registry coverage, field confidence, and the
  multi-project stack.
- [`scripts/demos/`](scripts/demos)
  Run narrow consumer-facing roundtrips for sockets, external engines, and
  training surfaces.
- [`scripts/linux/`](scripts/linux)
  Run Linux-only attach, kprobe, and tc smoke checks.
- [`scripts/perf/`](scripts/perf)
  Run targeted benchmark wrappers and local maintenance helpers.
- [`scripts/history/`](scripts/history)
  Render history artifacts such as minor-line IR snapshots.

## Goal To Script

### I want the fastest release answer

Run:

```bash
bash scripts/packaging/release_gate.sh
```

This is the highest-signal single entrypoint. It rebuilds native artifacts,
runs packaged release validation, and then runs the three-module stack smoke.

If you only want the packaged part, run:

```bash
bash scripts/packaging/release_container_check.sh
```

Relevant docs:

- [docs/release-checklist.md](docs/release-checklist.md)
- [docs/packaging.md](docs/packaging.md)

### I want to verify the packaged Linux artifacts

Run:

```bash
bash scripts/packaging/package_install_smoke.sh
bash scripts/packaging/container_runtime_validation.sh
bash scripts/packaging/container_validation_summary.sh
```

Use these when the question is specifically about `deb`/`rpm` output rather
than source-tree behavior.

### I want to validate built-in protocol packages

Run:

```bash
cargo run --quiet --bin gewyvern_validate -- registry
cargo run --quiet --bin gewyvern_validate -- high-frequency
cargo run --quiet --bin gewyvern_validate -- debugger-cross
```

Use `gewyvern_validate registry` for per-package drift, and
`gewyvern_validate high-frequency` for the practical high-traffic protocol
shelf.
Use `gewyvern_validate debugger-cross` when you want debugger confidence rather
than only package confidence: the Rust-native harness cross-checks summary
JSON, debugger-console JSON, and `gewyc` envelope output, then runs negative
cases that must stay in collect-more-evidence posture instead of pretending to
be actionable. The legacy
`scripts/validation/registry_validation.sh` and
`scripts/validation/high_frequency_validation.sh` and
`scripts/validation/debugger_cross_validation.sh` entrypoints are now thin
compatibility wrappers around the native commands.

Relevant docs:

- [docs/field-validation.md](docs/field-validation.md)
- [docs/book/how-to-add-or-debug-protocol-package.md](docs/book/how-to-add-or-debug-protocol-package.md)

### I want to validate live `--serve` behavior

Run:

```bash
cargo run --quiet --bin gewyvern_validate -- runtime-operator
cargo run --quiet --bin gewyvern_validate -- field-smoke --socket --scan-all
cargo run --quiet --bin gewyvern_validate -- runtime-lifecycle
cargo run --quiet --bin gewyvern_validate -- resilience-roundtrip
cargo run --quiet --bin gewyvern_validate -- resilience-log-evidence --log-source /path/to/runtime.log
cargo run --quiet --bin gewyvern_validate -- resilience-bundle --api-addr 127.0.0.1:9910 --log-source /path/to/runtime.log
cargo run --quiet --bin gewyvern_validate -- resilience-emit-helper --mode fail --output /tmp/gewyvern-external-fail.sh
cargo run --quiet --bin gewyvern_validate -- resilience-drive-bad-json --host 127.0.0.1 --port 9909 --count 6
bash scripts/validation/runtime_resilience_fault_injection.sh --help
bash scripts/validation/runtime_resilience_roundtrip.sh
bash scripts/validation/runtime_resilience_log_evidence.sh /path/to/runtime.log
bash scripts/validation/runtime_resilience_validation.sh 127.0.0.1:9910 /path/to/runtime.log
```

Use this when you care about:

- socket ingest surviving bad input
- startup, explicit stop, log evidence, and temporary run-dir cleanup
- latest-summary, export, analysis, and training dataset API readability
- read-only API behavior
- latest snapshot, analysis, export, and training surfaces

The legacy `scripts/validation/runtime_lifecycle_validation.sh` entrypoint
remains as a compatibility wrapper around
`gewyvern_validate runtime-lifecycle`.
- operator-facing deployment posture

Relevant docs:

- [docs/book/how-to-validate-runtime-surface.md](docs/book/how-to-validate-runtime-surface.md)
- [docs/book/how-to-security-checklist.md](docs/book/how-to-security-checklist.md)
- [docs/book/how-to-fault-inject-runtime-resilience.md](docs/book/how-to-fault-inject-runtime-resilience.md)

### I want to validate the real multi-project stack

Run:

```bash
bash scripts/validation/three_module_stack_smoke.sh
```

When validating on a reused physical or CI host that already has a suitable
Linux development image, skip the Docker rebuild and refresh the leserpent
NuGet graph explicitly:

```bash
IMAGE_TAG=gewyvern-stack-dev-physical \
  SKIP_DOCKER_BUILD=true \
  LESERPENT_DOTNET_RESTORE_FIRST=true \
  LESERPENT_DOTNET_IGNORE_FAILED_SOURCES=true \
  LESERPENT_DOTNET_NO_RESTORE=true \
  bash scripts/validation/three_module_stack_smoke.sh
```

This is the current collaboration smoke across:

- two nearby `gewyvern` runtimes
- one `etragon` sidecar
- one `leserpent` control plane
- one resilience-contract check per `gewyvern` runtime

Use it when the question is about protocol support plus cross-project
contracts, sidecar visibility, and control-plane registration semantics.

The script now expects each runtime to publish a healthy
`/v1/runtime/resilience.json` surface before the stack is considered ready, so
the control-plane handoff is validated at the contract level instead of only at
the process-health level. Its JSON readiness checks are now delegated to
`gewyvern_validate stack-probe` and `stack-check-json`, while the shell layer
keeps only the Docker, `dotnet`, and HTTP mutation orchestration.

It also injects repeated bad socket input into one runtime and verifies that:

- `/health` flips `resilience_degraded` to `true`
- `/v1/runtime/resilience.json` moves to `status = "degraded"`
- the degraded posture stays specific to socket backoff instead of falsely
  implying external-analysis failure

On success it also prints a `resilience_summary=...` path that points to a
small archive-friendly text summary for the healthy and degraded phases.

If you want that file to land somewhere durable instead of under the temporary
work directory, set `RESILIENCE_SUMMARY_PATH=/absolute/path/to/file.txt`
before running the script.

### I want a narrow consumer roundtrip

Run one of:

```bash
cargo run --quiet --bin gewyvern_validate -- socket-roundtrip --socket-target /tmp/gewyvern.sock --template udp --output /tmp/gewyvern-out.json --socket-kind unix
cargo run --quiet --bin gewyvern_validate -- external-engine-roundtrip --ingest-addr 127.0.0.1:9900 --api-addr 127.0.0.1:9910 --template udp --analysis-out /tmp/gewyvern-analysis.json --engine-out /tmp/external-engine-augmentations.json
cargo run --quiet --bin gewyvern_validate -- training-roundtrip --api-addr 127.0.0.1:9910 --out-dir /tmp/gewyvern-training-roundtrip
```

Use these when you want one thin path instead of a grouped validation shelf.
The socket, external-engine, and training dataset shell demos remain legacy
wrappers around the native `gewyvern_validate` commands.

### I want Linux-only probe smoke

Run one of:

```bash
sudo bash scripts/linux/linux_attach_smoke.sh
sudo bash scripts/linux/linux_kprobe_smoke.sh
sudo bash scripts/linux/linux_tc_smoke.sh <default-route-device>
```

Use these only on Linux-capable environments with the required kernel support
and BPF attach privileges. Without root, `CAP_BPF`/`CAP_NET_ADMIN`, or an
equivalent lab setup, the loader can fail with `Operation not permitted` before
it reaches gewyvern-specific behavior.

### I want a local benchmark or history snapshot

Run:

```bash
bash scripts/perf/benchmark_summary.sh
bash scripts/perf/trim_workspace_disk.sh --dry-run
bash scripts/perf/trim_workspace_disk.sh
bash scripts/history/render_minor_line_ir_snapshot.sh v0.15.x
```

Use `trim_workspace_disk.sh` when local iteration has left behind large
rebuildable artifacts. It removes:

- Rust `target/`
- frontend `node_modules/`
- .NET `bin/` and `obj/`
- Python cache directories such as `__pycache__/`

The script intentionally skips source, docs, Git history, and
`apps/leserpent/src/Leserpent/data`.

## Suggested Reading Order

If you are new to the project and want to orient first, use:

1. [README.md](README.md)
2. [docs/index.md](docs/index.md)
3. [docs/script-entrypoints.md](docs/script-entrypoints.md)
4. [docs/field-validation.md](docs/field-validation.md)
5. [docs/release-checklist.md](docs/release-checklist.md)

That sequence gives you the product posture, the docs map, the script map, the
current validation posture, and the actual ship gate.
