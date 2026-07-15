# Script Entrypoints

This page is the shortest goal-based map for the reorganized `scripts/` tree.

Use it when you know what you want to prove, but do not want to remember which
script shelf currently owns that check.

The naming split used throughout the repository is:

- `roundtrip`: one narrow end-to-end consumer path
- `smoke`: one lightweight bring-up or existence check
- `validation`: one grouped expectation check
- `summary`: one wrapper over narrower validations

## JSON Mode

Most native `gewyvern_validate` entrypoints now support a global `--json` flag:

```bash
cargo run --quiet --bin gewyvern_validate -- --json list
cargo run --quiet --bin gewyvern_validate -- --json help
```

Use this when the caller is CI, a release bot, or a local wrapper that should
not scrape human-facing log lines.

Current JSON behavior:

- success paths emit one final JSON object on stdout
- failure paths emit one final JSON object on stderr
- `release-gate`, packaged validation, and remote-host validation now suppress
  their normal progress chatter on stdout while `--json` is active
- human-facing text mode remains unchanged when `--json` is not present

If you want the final JSON result written to a file as well, add the global
flag before the command:

```bash
cargo run --quiet --bin gewyvern_validate -- --json --json-out /tmp/gewyvern-release-gate.json release-gate
```

Place the global `--json-out <path>` before the subcommand. This keeps
`runtime-operator --json-out <path>` available for its existing per-command
summary file behavior.

Stable top-level fields today:

- `schema_version`
- `ok`
- `command`
- `name`
- `checks`
- `evidence_dir`
- `extra`

Current rule:

- `schema_version = 1`

Machine consumers should gate parser behavior on `schema_version` before
assuming newer `extra.*` fields exist.

For machine consumers, prefer `extra` over parsing text summaries like
`slowest-phases:` or `covered-checks:`.

Current JSON failure codes:

| `failure_code` | Meaning | Typical action |
| --- | --- | --- |
| `invalid_cli_input` | a required option is missing, malformed, or unknown | rerun with `gewyvern_validate help` or the subcommand `--help` output |
| `docker_unreachable` | Docker is installed but the daemon is not reachable | start Docker Desktop or another daemon, then retry |
| `missing_package_artifact` | packaged validation could not find a local `.deb` or `.rpm` artifact | rebuild packages first, then rerun the packaged command |
| `validation_timeout` | one validation phase timed out or a process never exited cleanly | rerun a narrower command and inspect the corresponding evidence |
| `remote_workspace_retained` | a remote-host run failed after creating a remote workspace | SSH in and inspect the retained remote directory |
| `remote_host_not_linux` | the chosen remote host is not Linux | rerun against a Linux host |
| `remote_host_wrong_arch` | the chosen remote host is not `x86_64` / `amd64` | rerun against a supported Linux architecture |
| `remote_admin_credentials_incomplete` | only one of the remote admin credential env vars was set | set both `GEWY_REMOTE_EBPF_ADMIN_USER` and `GEWY_REMOTE_EBPF_ADMIN_PASSWORD`, or unset both |
| `linux_ebpf_privilege_required` | Linux eBPF attach smoke lacked a Linux/BPF-privileged environment | rerun on Linux with `sudo` or equivalent privileges |
| `missing_sshpass` | the optional admin-assisted remote eBPF path was requested without `sshpass` installed | install `sshpass`, or disable the admin-assisted path |
| `missing_system_command` | a required system command such as `ssh`, `rsync`, or `docker` is missing | install the missing command and rerun |

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

On macOS, container shell entrypoints dispatch to the trusted Linux server by
default. They synchronize evidence back into the local `target/` tree. The
policy, overrides, and privilege boundary are documented in
[remote Docker execution](remote-docker.md).

Relevant docs:

- [docs/release-checklist.md](docs/release-checklist.md)
- [docs/packaging.md](docs/packaging.md)

### I want to verify the packaged Linux artifacts

Run the shell wrappers below when Docker should execute on the configured Linux
server. Direct `cargo run ... container-*` commands remain host-local by design.

```bash
bash scripts/packaging/container_runtime_validation.sh
bash scripts/packaging/container_protocol_validation.sh
bash scripts/packaging/container_operator_path_validation.sh
bash scripts/packaging/container_validation_summary.sh
```

Use these when the question is specifically about `deb`/`rpm` output rather
than source-tree behavior.

The protocol/operator container checks are now native `gewyvern_validate`
commands. Their `scripts/packaging/*.sh` entrypoints remain as thin
compatibility wrappers.

### I want to validate on a real Linux host

Run:

```bash
cargo run --quiet --bin gewyvern_validate -- remote-linux-host-validation
```

This syncs the current workspace to a remote Linux host over SSH, builds
`x86_64` packages there, then runs host-mode package and runtime smoke checks.
Before any package/run step, it records a remote preflight snapshot so failures
separate environment drift from runtime regressions.
It also records Linux eBPF smoke evidence: when passwordless `sudo` and a
default-route device are available, it runs the native attach/kprobe/tc smokes;
otherwise it records an explicit `skipped` reason instead of turning an
environment privilege gap into a false runtime regression.
The remote packaging and remote eBPF validator paths now reuse a shared remote
Cargo target cache under `~/.cache/gewyvern/remote-target`, so repeated runs do
not have to cold-rebuild every binary from a brand-new workspace.
They also reuse a shared remote source cache under
`~/.cache/gewyvern/remote-source`: the local machine rsyncs incrementally into
that stable cache first, then each validation run repoints its requested remote
workspace path at that cache on the remote host itself instead of copying the
same tree twice.
The workspace sync for this command is intentionally narrower than the full
monorepo: it skips `tests/`, transient `apps/**/bin/` / `apps/**/obj/` outputs,
`__pycache__`, and similar local-only residue because the remote host package
and runtime checks do not consume those shelves.
When that filtered workspace snapshot is unchanged, the command now reuses a
workspace sync cache marker and skips the rsync phase entirely.

If the SSH user cannot `sudo -n` but you do have a separate admin account,
export `GEWY_REMOTE_EBPF_ADMIN_USER` and `GEWY_REMOTE_EBPF_ADMIN_PASSWORD`
before running the command. That credential path is used only for the remote
eBPF attach step; the normal workspace sync and package/runtime flow stay on
the existing SSH path.

Defaults:

- host from `GEWY_REMOTE_HOST` or `kyuubiki-lab`
- remote workspace under `~/.kyuubiki-remote-runs/`

Useful flags:

- `--host <ssh-host>`
- `--remote-dir <path>`
- `--skip-build`
- `--keep-remote-dir`

Evidence written locally:

- `target/validation/remote-linux-host-validation/remote-preflight.txt`
- `target/validation/remote-linux-host-validation/remote-artifacts.txt`
- `target/validation/remote-linux-host-validation/remote-package-build-timings.txt`
- `target/validation/remote-linux-host-validation/remote-package-smoke-timings.txt`
- `target/validation/remote-linux-host-validation/remote-runtime-smoke-timings.txt`
- `target/validation/remote-linux-host-validation/remote-ebpf.txt`
- `target/validation/remote-linux-host-validation/remote-ebpf/`
- `target/validation/remote-linux-host-validation/remote-phase-timings.txt`
- `target/validation/remote-linux-host-validation/remote-run.txt`
- `target/validation/remote-linux-host-validation/remote-ebpf-history.jsonl`
- `target/validation/remote-linux-host-validation/remote-ebpf-latest.json`
- `target/validation/remote-linux-host-validation/remote-ebpf-recent.txt`
- `target/validation/remote-linux-host-validation/remote-ebpf-status-summary.json`

The phase-timing file records the observed wall-clock time for each major
remote validation step so we can tell whether regressions come from sync,
materialization, build, package smoke, runtime smoke, or the privileged eBPF
attach path.
Artifact verification now prefers the package build manifest emitted under
`target/packages/build-manifest.txt` instead of rescanning the package
directories on every run.
The package smoke path now also emits a subphase timing file and uses
content-stamped unpack caches for both DEB and RPM payloads, so repeated runs
reuse verified package trees without silently masking changed artifacts.
The runtime smoke path also emits a subphase timing file so we can distinguish
package unpack cache refresh from TCP/UDP boot, summary, and analysis waits.
The eBPF history files keep a bounded local record of the newest remote Linux
eBPF outcomes so we can tell whether the attach path is consistently `ok`,
frequently `skipped`, or drifting in total runtime.
`remote-ebpf-recent.txt` gives a compact last-five human view, while
`remote-ebpf-status-summary.json` rolls up counts by status and reason.
The CLI now also prints a compact post-run summary with the resolved remote
workspace, source/target cache roots, remote eBPF result, and the slowest
slowest observed phases so the common debugging path does not require opening the
evidence files first.
It also prints the remote kernel, the detected default-route device for the tc
smoke, and the total observed wall-clock seconds for the full remote run.
When keyed remote phases materially exceed the current soft baseline budgets,
the summary also prints `budget-warnings:`.
That currently includes the full `total`, `workspace_sync`,
`remote_package_build`, `remote_package_smoke`, `remote_runtime_smoke`,
`remote_ebpf_smoke`, and `remote_ebpf_evidence_sync` phases.
When local remote-eBPF history exists, the summary also prints a compact recent
trend line plus the newest recent-history entries.

For machine-readable consumption, use:

```bash
cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation
```

The `extra` object for this command now includes structured fields such as:

- `remote_dir`
- `source_cache`
- `target_cache`
- `build_packages_enabled`
- `keep_remote_dir`
- `remote_checks`
- `preflight`
- `ebpf`
- `phase_timings`
- `package_build_timings`
- `package_smoke_timings`
- `runtime_smoke_timings`
- `total_seconds`
- `slowest_phase_entries`
- `budget_warnings`
- `validation_posture`
- `release_gate_signal`
- `next_step`
- `linux_proof_complete`
- `requires_followup`
- `remote_ebpf_history_entries`
- `remote_ebpf_status_counts`
- `remote_ebpf_reason_counts`
- `recent_ebpf_trend`
- `recent_ebpf_lines`

Example `jq` checks:

```bash
cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation \
  | jq '.extra.preflight.arch == "x86_64"'

cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation \
  | jq '.extra.ebpf.status'

cargo run --quiet --bin gewyvern_validate -- --json remote-linux-host-validation \
  | jq '.extra.slowest_phase_entries[0]'
```

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
JSON, debugger-console JSON, debug-session `debugger_posture`, and `gewyc`
envelope output, then writes `evidence-index.json` as the compact case map.
That index includes `debugger_route` so release review can see the safe next
surface without opening every raw JSON file. It also runs negative cases that
must stay in collect-more-evidence posture instead of pretending to be
actionable. The legacy
`scripts/validation/registry_validation.sh` and
`scripts/validation/high_frequency_validation.sh` and
`scripts/validation/debugger_cross_validation.sh` entrypoints are now thin
compatibility wrappers around the native commands.

Relevant docs:

- [docs/field-validation.md](docs/field-validation.md)
- [docs/book/how-to-add-or-debug-protocol-package.md](docs/book/how-to-add-or-debug-protocol-package.md)

### I want CI-friendly release-gate output

Run:

```bash
cargo run --quiet --bin gewyvern_validate -- --json release-gate
```

Or, for narrow debugging:

```bash
cargo run --quiet --bin gewyvern_validate -- --json release-gate --skip-build
cargo run --quiet --bin gewyvern_validate -- --json release-gate --skip-stack
cargo run --quiet --bin gewyvern_validate -- --json release-gate --skip-debugger-cross
cargo run --quiet --bin gewyvern_validate -- --json release-gate --skip-pathology
cargo run --quiet --bin gewyvern_validate -- --json release-gate --remote-host-validation
```

The `extra` object for `release-gate` currently exposes:

- `stages.build_packages`
- `stages.release_container_check`
- `stages.three_module_stack_smoke`
- `stages.pathological_container_validation`
- `stages.remote_linux_host_validation`
- top-level `ship_signal = "timing_watch"` when the remote host passed but one
  of the soft timing budgets regressed
- `remote`

The practical Linux target-lab command
`juice-shop-container-validation`
is intentionally outside that default `release-gate` stage map today. Treat it
as an explicit high-signal companion artifact when you want stronger Linux/BPF
evidence, not as a stage that generic CI should silently assume.

The same companion-artifact pattern also applies to
`ftp-denied-container-validation` and
`ldap-bind-denied-container-validation` when you want explicit
authentication-denial evidence instead of suspicious HTTP target behavior.

Every successful `release-gate` run also refreshes:

- `target/validation/release-gate-artifacts.json`
- `target/validation/release-gate-artifacts.txt`

Use those two companion files as the compact directory-level index of which
release-facing evidence shelves are currently present under `target/validation/`,
including the separate optional `juice-shop-container` shelf when it exists.

`remote` is `null` unless the current run actually executed the remote-host
stage. This is deliberate so CI cannot accidentally read stale evidence from an
older local `target/validation/remote-linux-host-validation` directory.

Example:

```bash
cargo run --quiet --bin gewyvern_validate -- --json release-gate \
  | jq '.extra.stages'
```

### I want to validate live `--serve` behavior

Run:

```bash
cargo run --quiet --bin gewyvern_validate -- runtime-operator
cargo run --quiet --bin gewyvern_validate -- field-smoke --socket --scan-all
cargo run --quiet --bin gewyvern_validate -- runtime-lifecycle
cargo run --quiet --bin gewyvern_validate -- resilience-roundtrip
cargo run --quiet --bin gewyvern_validate -- resilience-log-evidence --log-source target/validation/runtime.log
cargo run --quiet --bin gewyvern_validate -- resilience-bundle --api-addr 127.0.0.1:9910 --log-source target/validation/runtime.log
cargo run --quiet --bin gewyvern_validate -- resilience-emit-helper --mode fail --output /tmp/gewyvern-external-fail.sh
cargo run --quiet --bin gewyvern_validate -- resilience-drive-bad-json --host 127.0.0.1 --port 9909 --count 6
bash scripts/validation/runtime_resilience_fault_injection.sh --help
bash scripts/validation/runtime_resilience_roundtrip.sh
bash scripts/validation/runtime_resilience_log_evidence.sh target/validation/runtime.log
bash scripts/validation/runtime_resilience_validation.sh 127.0.0.1:9910 target/validation/runtime.log
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
work directory, set `RESILIENCE_SUMMARY_PATH=target/validation/resilience-summary.txt`
before running the script.

### I want one practical Linux target-lab read

Run:

```bash
sudo cargo run --quiet --bin gewyvern_validate -- juice-shop-container-validation
```

Or through the server-aware wrapper:

```bash
bash scripts/validation/juice_shop_container_validation.sh
```

Use this when the real question is:

- can `gewyvern` preserve suspicious target-side evidence from a live Docker lab?
- can the same Linux host still prove tracepoint, kprobe, and tc attach health?
- do we have one repeatable practical-target shelf that is stronger than a synthetic demo?

If you want the same style of proof for protocol/authentication denial instead
of HTTP error evidence, run:

```bash
bash scripts/validation/ftp_denied_container_validation.sh
bash scripts/validation/ldap_bind_denied_container_validation.sh
```

These practical suites also contain same-host eBPF attach proof. Docker group
access is automatic on the validation account, but BPF privilege remains a
separate explicit requirement and is never silently elevated by the wrapper.

That companion check preserves client-side FTP `530` denial evidence,
target-side `FAIL LOGIN` server logs, and the same nested Linux attach proof.

The LDAP companion preserves client-side `ldap_bind: Invalid credentials (49)`
evidence, target-side `BIND ... err=49` logs, and the same nested Linux attach
proof on the same host.

What the current check proves:

- an OWASP Juice Shop container becomes reachable on a loopback-bound host port
- a file-guard style request preserves `Only .md and .pdf files are allowed!`
- a malformed SQL-style search preserves `SQLITE_ERROR: incomplete input`
- the same host still passes `linux-attach-smoke`, `linux-kprobe-smoke`, and `linux-tc-smoke`

The evidence shelf now also writes `evidence-index.json` as the compact map of
the target-side HTTP captures, container log, summary, and nested same-host
Linux attach evidence. Read that file first before drilling into the raw
artifacts.

What it does not prove:

- direct vulnerability classification by `gewyvern`
- complete web attack coverage
- authenticated or browser-driven exploit workflows

This is intentionally a Linux-only practical lab shelf because the attach proof
requires BPF attach privileges. Unprivileged runs may fail with `Operation not
permitted`.

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
sudo cargo run --quiet --bin gewyvern_validate -- linux-attach-smoke
sudo cargo run --quiet --bin gewyvern_validate -- linux-kprobe-smoke
sudo cargo run --quiet --bin gewyvern_validate -- linux-tc-smoke --dev <default-route-device>
```

Use these only on Linux-capable environments with the required kernel support
and BPF attach privileges. Without root, `CAP_BPF`/`CAP_NET_ADMIN`, or an
equivalent lab setup, the loader can fail with `Operation not permitted` before
it reaches gewyvern-specific behavior.

Each Linux smoke writes an evidence shelf under `target/validation/...` with:

- `target.txt`
- `run.log`
- `environment.txt`
- `evidence-index.json`
- `netdev.txt` for `linux-tc-smoke`

`environment.txt` records the kernel release/version, effective capability
mask, BPF-related filesystem presence, and whether `clang`, `cc`, `tc`, and
`bpftool` were discoverable in `PATH`. That makes attach failures much easier
to compare across local Linux hosts and remote validation runs.

The legacy `scripts/linux/*.sh` entrypoints remain as thin compatibility
wrappers around these native commands.

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
