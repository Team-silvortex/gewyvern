# Script Entrypoints

This page is the shortest goal-based map for the reorganized `scripts/` tree.

Use it when you know what you want to prove, but do not want to remember which
script shelf currently owns that check.

The naming split used throughout the repository is:

- `roundtrip`: one narrow end-to-end consumer path
- `smoke`: one lightweight bring-up or existence check
- `validation`: one grouped expectation check
- `summary`: one wrapper over narrower validations

## Native Developer Workflow

Routine build, package, and local desktop deployment no longer require callers
to compose the lower-level entrypoints by hand:

```bash
cargo dev doctor
cargo dev version check
cargo dev check
cargo dev build
cargo dev package linux --format layout
cargo dev package control
cargo dev deploy control --reuse --no-start
cargo dev package desktop
cargo dev deploy desktop --launch
```

`cargo dev version check` verifies the shared Cargo/.NET/lockfile/documentation
release identity. `cargo dev version set VERSION [--dry-run]` updates those
current-version surfaces transactionally while leaving historical evidence and
Git tags untouched. `cargo dev check` skips Rust code generation and linking
while retaining incremental managed builds; `cargo dev build` produces the
Rust, Leserpent control service, and Avalonia outputs. Test projects remain on
the explicit regression/release gates instead of the routine output build.
Both run their selected stacks in parallel with locked dependency behavior and
support narrow `--scope` values.
The package and deploy commands keep atomic output boundaries and report their
final artifact. Use the scripts and specialized Rust binaries below when
debugging an individual stage. Formal macOS release packaging uses the same
desktop command with the paired `--identity` and `--notary-profile` options.

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
| `host_permission_denied` | a non-eBPF local path or IPC operation was denied by the host | choose writable runtime/output paths or grant only the named host permission; do not assume eBPF or sudo |
| `missing_sshpass` | the optional admin-assisted remote eBPF path was requested without `sshpass` installed | install `sshpass`, or disable the admin-assisted path |
| `missing_system_command` | a required system command such as `ssh`, `rsync`, or `docker` is missing | install the missing command and rerun |
| `missing_native_aot_dependency` | `dotnet` or the Linux Xvfb control-smoke dependency is missing | install the named host dependency and rerun `leserpent-aot` |

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

To inspect the current Leserpent bundle and fold its Apple readiness report
into the same machine gate without running signing commands again:

```bash
cargo run --quiet --bin gewyvern_leserpent_release -- preflight \
  --app artifacts/leserpent-avalonia/Leserpent.app \
  > /tmp/leserpent-macos-preflight.json
cargo run --quiet --bin gewyvern_validate -- --json release-gate \
  --macos-release-preflight /tmp/leserpent-macos-preflight.json
```

The input is bounded and strictly cross-checked. A consistent blocked report is
retained as evidence and marks `extra.stages.macos_release_preflight_blocked`,
while malformed or contradictory evidence fails the command. When all product
and Linux stages pass but Apple credentials remain absent, the aggregate signal
is `apple_credentials_blocked`, never `ready`.
Reports from earlier product versions are rejected; do not edit their version
or reuse the retained fixture as proof for a newly packaged bundle.

The Apple preflight and release stages share one fixed system-tool inventory:
the six host commands are executed by absolute `/usr/bin` or `/usr/sbin` path
under a standard system `PATH`. `xcrun` additionally ignores process-local
`DEVELOPER_DIR` and `TOOLCHAINS` overrides, so the reviewed Xcode selection must
be configured through `xcode-select` and cannot change between inventory and
execution through caller environment drift.

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

This command defaults to `--target-kind physical`. Preflight probes both
`systemd-detect-virt --vm` and `--container`; it fails closed when
virtualization cannot be identified, rejects containers, and rejects any kind
that disagrees with the request. A VM therefore cannot silently enter the
physical-host evidence shelf.

For compatibility testing on a VM, run:

```bash
cargo run --quiet --bin gewyvern_validate -- remote-linux-host-validation \
  --target-kind vm --host gewyvern-jammy
```

VM evidence is written under `target/validation/remote-linux-vm-validation`.
Its matrix reports `release_eligible=false`; a clean in-budget run reports
`compatibility_only`, while integrity or timing warnings can still lower it to
`watch`. It can never report release-ready, regardless of VM or kernel breadth.
The combined release gate always requests a physical target and never consumes
this VM shelf.

This syncs the current workspace to a remote Linux host over SSH, builds
`x86_64` packages there, then runs host-mode package and runtime smoke checks.
With remote builds enabled, it also publishes the Leserpent control-plane
NativeAOT bundle for `linux-x64`, verifies the ELF payload inventory, and runs
the compiled service through health, registration-plan, token-bound
registration, and unified recovery requests. The proof fails if the pairing
secret reaches either the runtime state file or SQLite database. It then
publishes the Avalonia `linux-x64` NativeAOT client, bundles the current Rust
`leserpentd`, and runs the real Local Orchestra language-pack verifier through
private-CA loopback TLS. That proof downloads `pt-BR` without bearer/admin
headers, binds catalog digest/locale/version, performs the private
install/load/remove roundtrip, and proves daemon cleanup plus immediate restart.
The same native package then persists that live authority as a saved daemon,
reloads it through the production connection catalog and managed CA store, and
repeats the download through `DesktopLanguagePackSource.FromConnection`. A
separate wrong-CA request must fail before the selected CA succeeds; both the
catalog and CA digests must remain unchanged, and the credential-rejecting
public route proves that no bearer or admin header was sent.
`--skip-build` skips both NativeAOT phases together with package construction.
After rsync, the Rust harness strictly revalidates the synchronized evidence:
the bounded index and exact file inventory, regular non-symlink file types,
health and token shapes, recovery semantics, attention action, payload hash
inventory, and secret-free registration response must all agree before the
phase is reported as covered. The isolated runtime state JSON and checkpointed
SQLite database are retained in the evidence shelf as bounded files; the local
Rust validator independently parses their formats and scans every synchronized
artifact for the proof pairing secret.
Both the regular CLI summary and the combined release gate repeat this strict
validation at evidence-consumption time. The language-pack shelf has separate
fixed-contract `verification.log` and `saved-verification.log` files, so a Local
Orchestra pass cannot substitute for a missing saved-daemon pass. Current JSON
summaries expose
`leserpent_control_plane_aot_evidence_validated=true` and
`leserpent_language_pack_local_orchestra_aot_evidence_validated=true`;
replacing either shelf between the remote run and summary rendering therefore
fails closed. The language-pack validator also recomputes the current local
catalog and `pt-BR` asset hashes, so stale synchronized content cannot satisfy
the gate.
Before the release build it runs a cached Linux
`cargo check --workspace --all-targets` over the filtered workspace, catching
target-specific library, binary, example, benchmark, and inline-test compile
drift, including the root integration-test targets and Linux-only eBPF tests.
It first runs the stricter locked `cargo clippy --workspace --all-targets -- -D
warnings` quality phase against the same target cache. Remote release preflight
therefore requires the official `cargo-clippy` component and fails explicitly
when that component is absent instead of silently downgrading to compile-only
coverage.
Before any package/run step, it records a remote preflight snapshot so failures
separate environment drift from runtime regressions.
The snapshot and bounded history entry include the observed Rust, Cargo,
`dpkg-deb`, RPM, and `rpmbuild` version lines, making toolchain drift visible
without retaining unbounded command output.
Remote preflight, artifact, and eBPF key/value evidence share one strict parser:
inputs are capped at 8 KiB and 32 lines, keys must be unique and known, control
characters are rejected, and required values cannot be empty.
It also records Linux eBPF smoke evidence. The preferred path invokes the
root-owned `/usr/libexec/gewyvern-ebpf-helper` through a command-limited
`sudoers` rule. The helper accepts only `probe`, bounded
`run --run-id ... --device ...`, and bounded `cleanup --run-id ...`
operations; it never invokes a shell or caller-provided binary. Evidence is
created below the root-owned `/var/lib/gewyvern-ebpf-validation` directory and
copied into the workspace only after attach, kprobe, and tc smokes pass.
Preflight additionally requires helper protocol `1` and an exact match with
the current Gewyvern package version. A stale helper is recorded as unavailable
instead of being trusted to produce current-release evidence; its observed
version remains visible as `ebpf_helper_version` in preflight summaries.
Without a ready helper or an explicit admin fallback, validation records a
specific `privileged_helper_missing`, `privileged_helper_unavailable`, or
`privileged_helper_incompatible` skip rather than a false runtime regression.
The paired `ebpf_helper_state` field is restricted to `missing`, `unavailable`,
`incompatible`, or `ready`; contradictory availability evidence is rejected.
The remote packaging and remote eBPF validator paths now reuse a shared remote
Cargo target cache under `~/.cache/gewyvern/remote-target`, so repeated runs do
not have to cold-rebuild every binary from a brand-new workspace.
They also reuse a shared remote source cache under
`~/.cache/gewyvern/remote-source`: the local machine rsyncs incrementally into
that stable cache first, then each validation run repoints its requested remote
workspace path at that cache on the remote host itself instead of copying the
same tree twice.
The workspace sync for this command is intentionally narrower than the full
monorepo: it skips transient `apps/**/bin/` / `apps/**/obj/` outputs,
`__pycache__`, and similar local-only residue because the remote host package
and runtime checks do not consume those shelves. Root integration tests and
nested crate/application test modules remain synchronized because the Linux
all-target compile proof consumes them; the complete test shelf adds only about
1.6 MiB to a cold sync.
When that filtered workspace snapshot is unchanged, the command now reuses a
workspace sync cache marker and skips the rsync phase entirely.
Immediately after sync and again after eBPF evidence collection, the ordinary
SSH identity performs a bounded ownership scan over both shared caches. A
symlink cache root or any entry owned by a different numeric UID/GID fails the
run before the cache can be reused or left behind as hidden privileged residue.

If the fixed helper is not installed but you do have a separate admin account,
export `GEWY_REMOTE_EBPF_ADMIN_USER` and `GEWY_REMOTE_EBPF_ADMIN_PASSWORD`
before running the command. Preflight, workspace sync, caches, package/runtime
checks, validator builds, evidence retrieval, and cleanup always retain the
ordinary SSH identity selected by `GEWY_REMOTE_HOST`. The password-authenticated
admin identity is used only for the three kernel attach commands when the fixed
helper is unavailable. The sudo process reads the synchronized workspace's
numeric owner through `stat`, rejects malformed ownership values, and restores
the eBPF evidence directory to that owner on every exit path. This prevents an
admin fallback from moving source, caches, or build products into the admin
home while keeping `ssh` and `rsync` aligned on the workspace identity. Those
privileged smoke commands materialize only the five allowlisted sources embedded
at build time into a fresh private work directory. Neither the remote current
directory nor a changed/deleted build workspace can substitute loader or BPF
source files after installation. Existing files and symlinks are rejected.
This is a compatibility path. New hosts should install the fixed helper and
must not grant the validation account unrestricted passwordless `sudo`, or
authorize `env`, a shell, or workspace binaries in `sudoers`.
Packaged hosts can configure the fixed path with
`sudo gewyvern-ebpf-provision --user VALIDATION_USER`; add `--dry-run` to
validate the account and installed helper without writing configuration. This
root-only management binary is deliberately absent from the generated sudoers
allowlist.
The generated policy uses only terminal standalone `*` arguments so it also
parses with current `sudo-rs`. It still names only the fixed helper's `probe`,
`run --run-id ...` and `cleanup --run-id ...` prefixes. The helper, not wildcard
matching, enforces exact argument count/order, `--device`, bounded identifiers,
caller UID and root-owned paths. Never replace this with unrestricted sudo or a
shell wrapper. Existing policies containing `run --run-id * --device *` must be
regenerated with the current native provisioner and checked with the active
`visudo` before installation. See the upstream
[sudo-rs compatibility restrictions](https://github.com/trifectatechfoundation/sudo-rs#compatibility)
for argument-wildcard support; older implementations can still refuse the policy,
in which case provisioning must fail without replacing the existing file.
One run can cover package/runtime smoke, the Leserpent
control-plane NativeAOT persistence proof, and attach/kprobe/tc in one evidence
transaction. A first run under a new workspace identity may report `watch`
because its isolated source and target caches are cold; rerun with the same
ordinary identity to measure the warm reference. A clean warm run reports
`linux_proof_complete=true`. For the 2.0 community-release posture, a complete
physical-host run with `matrix.release_eligible=true` may report `ready` while
`matrix.ready=false`; the second physical host and second kernel release remain
an explicit post-2.0 breadth advisory rather than being hidden or treated as a
core-release blocker.

Defaults:

- host from `GEWY_REMOTE_HOST` or the dedicated `gewyvern-lab` SSH alias
- target kind `physical`
- remote workspace under `~/.gewyvern-remote-runs/`

The native validator multiplexes SSH through a nonce-bearing short control
socket under `/tmp` on Unix. It reserves space for OpenSSH's temporary suffix,
rejects overlong or unbounded control-path tokens, and shell-quotes the path
when handing it to rsync. `GEWY_SSH_CONTROL_PATH_TEMPLATE` may override the
template only with an absolute path using bounded `%C` or `%%` tokens.

The Linux control-plane NativeAOT proof supplies the same `PublishAot` and
`RuntimeIdentifier=linux-x64` MSBuild properties to locked restore and publish.
It deliberately avoids a publish-only `-r` shortcut, so a cold restore cannot
produce a different runtime graph from the later `--no-restore` publish. Its
SQLite proof binds writability to the live control-plane writer fence: schema
initialization occurs after lease acquisition, and later lease loss returns new
operations to read-only mode.

The companion language-pack proof derives the expected official pack version
and translation-leaf count from the current catalog and `pt-BR` asset before it
starts. Remote verification therefore cannot drift behind a pack expansion.
Failure cleanup emits the active publish, local-orchestra, saved-daemon, or
restore stage and a bounded log tail while still removing the large NativeAOT
temporary directories.

Useful flags:

- `--host <ssh-host>`
- `--target-kind <physical|vm>`
- `--remote-dir <path>`
- `--skip-build`
- `--keep-remote-dir`

Global `--json-out PATH` enables machine output without also requiring
`--json`. A write failure is fatal rather than being reported as a successful
validation. For example:

```bash
cargo run --quiet --bin gewyvern_validate -- \
  --json-out target/validation/remote-linux-host-validation/latest-run.json \
  remote-linux-host-validation
```

Evidence written locally:

- `target/validation/remote-linux-host-validation/remote-preflight.txt`
- `target/validation/remote-linux-host-validation/remote-artifacts.txt`
- `target/validation/remote-linux-host-validation/remote-package-build-timings.txt`
- `target/validation/remote-linux-host-validation/remote-package-smoke-timings.txt`
- `target/validation/remote-linux-host-validation/remote-runtime-smoke-timings.txt`
- `target/validation/remote-linux-host-validation/remote-ebpf.txt`
- `target/validation/remote-linux-host-validation/remote-ebpf/`
- `target/validation/remote-linux-host-validation/leserpent-control-plane-aot-linux-x64/`
- `target/validation/remote-linux-host-validation/leserpent-language-pack-local-orchestra-native-aot-linux-x64/`
- `target/validation/remote-linux-host-validation/remote-phase-timings.txt`
- `target/validation/remote-linux-host-validation/remote-run.txt`
- `target/validation/remote-linux-host-validation/remote-ebpf-history.jsonl`
- `target/validation/remote-linux-host-validation/remote-ebpf-history-rejected.jsonl`
- `target/validation/remote-linux-host-validation/remote-ebpf-latest.json`
- `target/validation/remote-linux-host-validation/remote-ebpf-recent.txt`
- `target/validation/remote-linux-host-validation/remote-ebpf-status-summary.json`
- the path supplied through `--json-out`, such as
  `target/validation/remote-linux-host-validation/latest-run.json`

With `--target-kind vm`, the same bounded inventory is written under
`target/validation/remote-linux-vm-validation/` instead. The two complete run
locks, histories, latest records, and matrix summaries never share a path.
The local workspace-key optimization is not evidence and is isolated separately
under `target/validation/remote-workspace-sync-cache/{physical,vm}.txt`.

The phase-timing file records the observed wall-clock time for each major
remote validation step so we can tell whether regressions come from sync,
materialization, build, the Leserpent control-plane NativeAOT proof, package
smoke, the Avalonia Local Orchestra plus saved-daemon language-pack NativeAOT
proof, runtime smoke, or the privileged eBPF attach path.
Remote build/package/runtime subphase timing files are mandatory after their
corresponding successful stage. They use the shared bounded unique-key parser,
accept only known phases and finite non-negative values up to 24 hours, and
require every non-cache phase before canonical local persistence.
The CLI summary then reuses the same bounded unique-key codec when reading
local evidence. It rejects non-regular files, files over 8 KiB, schema drift,
duplicate keys, invalid booleans, and invalid timing values instead of emitting
a partially populated success summary.
Artifact verification now prefers the package build manifest emitted under
`target/packages/build-manifest.txt` instead of rescanning the package
directories on every run. Local packaged container checks use the same
manifest-bound selection and reject duplicate keys, stale filename ordering,
out-of-root paths, symlinks, and extension-shaped directories.
Remote artifact collection, package smoke, and runtime smoke use one strict
resolver, so no remote entrypoint can fall back to first-match manifest parsing.
The package smoke path now also emits a subphase timing file and uses
content-stamped unpack caches for both DEB and RPM payloads, so repeated runs
reuse verified package trees without silently masking changed artifacts.
The runtime smoke path also emits a subphase timing file so we can distinguish
package unpack cache refresh from TCP/UDP boot, summary, and analysis waits.
The eBPF history files keep a bounded local record of the newest remote Linux
eBPF outcomes so we can tell whether the attach path is consistently `ok`,
frequently `skipped`, or drifting in total runtime.
`remote-ebpf-recent.txt` gives a compact last-five human view, while
`remote-ebpf-status-summary.json` rolls up counts by status and reason. Its
`matrix` object counts successful distinct hosts, kernels, and architectures;
`matrix.ready` requires at least two hosts and two kernel releases, so repeated
success on one machine cannot masquerade as broad physical-host coverage.
The summary also records `target_kind`, `matrix.breadth_ready`, and
`matrix.release_eligible`. VM breadth can exercise portability, but
`matrix.ready` is always false for VM evidence. A successful physical-host run
with `matrix.release_eligible=true` can report `release_gate_signal=ready` and
`requires_followup=false` even when the broader matrix is not yet ready; the
matrix fields and `next_step` preserve that post-2.0 breadth advisory. A false
or missing release-eligibility signal keeps the stricter
`coverage_incomplete` result.
Physical hosts are keyed by a SHA-256 digest of the remote machine ID; the raw
machine ID is never stored or returned. Records without a valid digest remain
readable but appear under `unidentified_successful_runs` and do not increase
matrix breadth, so SSH aliases cannot forge additional hosts.
History updates are bounded and atomically replaced after file and directory
sync. Every retained entry must satisfy the versioned core schema and finite
timing constraints. Invalid, truncated, or malformed lines are removed from
the active history and preserved in `remote-ebpf-history-rejected.jsonl` for
audit instead of silently affecting retention or trend totals. The summary's
`integrity` object reports `clean` or `repaired`; repaired history changes the
release signal to `watch` until the rejected evidence has been reviewed.
Validators targeting the shared local evidence shelf serialize the complete
remote run, so preflight, timings, latest evidence, and summary files cannot be
mixed between processes. A contender waits for up to two minutes; a run lock
older than 15 minutes is treated as crash residue. The final history merge also
has its own short critical-section lock with a five-second wait and 30-second
stale threshold. Default remote workspaces include the local process ID and a
per-process sequence, so runs started in the same second do not share or clean
up each other's remote directory.
The CLI now also prints a compact post-run summary with the resolved remote
workspace, source/target cache roots, and remote eBPF result. It includes the
slowest observed phases so the common debugging path does not require opening
the evidence files first.
It also prints the remote kernel, the detected default-route device for the tc
smoke, and the total observed wall-clock seconds for the full remote run.
When keyed remote phases materially exceed the current soft baseline budgets,
the summary also prints `budget-warnings:`.
That currently includes the full `total`, `workspace_sync`,
`remote_package_build`, `remote_package_smoke`, `remote_runtime_smoke`,
`remote_ebpf_validator_build`, `remote_ebpf_attach`, and
`remote_ebpf_evidence_sync` phases. Historical evidence using the combined
`remote_ebpf_smoke` phase remains readable.
When local remote-eBPF history exists, the summary also prints a compact recent
trend line plus the newest recent-history entries.
The local summary reader treats synchronized evidence as untrusted input: keyed
files must use the strict unique-key schema, the history summary must be a
regular non-symlink JSON file no larger than 64 KiB, and recent evidence is
limited to five nonempty 512-byte lines in a regular file no larger than 16 KiB.
Malformed, missing, ambiguous, or oversized evidence fails the command instead
of silently producing a partial summary.
The combined release gate reuses these same bounded readers and validates all
remote evidence before printing a successful human summary, so its console
output cannot become a lenient alternative to the machine-readable result.

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
- `leserpent_control_plane_aot_evidence_validated`
- `leserpent_language_pack_local_orchestra_aot_evidence_validated`
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
- `remote_ebpf_history_integrity`
- `remote_ebpf_status_counts`
- `remote_ebpf_reason_counts`
- `remote_ebpf_matrix`
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

### I want to replay the Leselang fuzz shelf

Run:

```bash
cargo run --quiet --bin gewyvern_validate -- leselang-fuzz
```

The fixed seed runs 2048 UTF-8 parser/HIR/VM source cases and 2048 mutated
continuation decoder cases. It checks lossless reconstruction, character-safe
spans, deterministic diagnostics and JSON roundtrips, bounded VM startup, and
fail-closed continuation decoding without requiring nightly Rust or an external
fuzzer. Evidence is retained under `target/validation/leselang-fuzz/` as
`fuzz-config.json`, `run.log`, and `evidence-index.json`.

### I want to prove Leserpent UI accessibility

Run:

```bash
cargo run --quiet --bin gewyvern_validate -- leserpent-accessibility
```

The command performs a locked managed build and opens all four Avalonia
renderer fixtures plus six product function-chain probes on the supported
native host. The product probes exercise Hub topology, the remote shell
(including registration, Orchestra projection, and debugger re-entry), daemon
bootstrap, Gewyvern provisioning, Gewyvern retirement, and daemon retirement.
It verifies strict terminal success markers in addition to unique Automation
IDs, complete names, explicit action labels, HelpText mapping, and the 4.5 WCAG
AA text-contrast floor. Linux uses `xvfb-run`; the validator never installs it
or invokes `sudo`. The schema-v2 summary and separate fixture/probe logs are
retained under `target/validation/leserpent-accessibility/`.

### I want to prove the Leserpent native desktop artifact

Run:

```bash
cargo run --quiet --bin gewyvern_validate -- leserpent-aot
```

This host-native command supports the checked `osx-arm64` and `linux-x64`
targets. It performs a locked multi-RID restore, publishes with `--no-restore`,
checks the Mach-O or ELF signature, bounds the artifact file set, and runs all
four real Avalonia control fixtures. Linux requires `xvfb-run` and `xauth`; the
validator reports their absence but never installs packages or invokes `sudo`.

Evidence is retained under `target/validation/leserpent-aot/`:

- `environment.txt`
- `restore.log` and `publish.log`
- `artifact-manifest.json`
- one log for each control fixture
- `evidence-index.json`

Use global `--json` and `--json-out` when CI should consume the result without
parsing human output. Windows remains outside this command until `win-x64` has
a locked dependency graph and native-host execution evidence.

### I want to package or deploy the Leserpent Linux control service

On a Linux x86-64 builder, produce the complete content-addressed NativeAOT
control bundle with:

```bash
cargo dev package control
```

Build and atomically install it, or skip compilation and install an existing
bundle only after complete revalidation:

```bash
cargo dev deploy control
cargo dev deploy control --reuse --no-start
```

Restore and Rust payload construction overlap. Rust bridge and daemon
construction uses one Cargo invocation; managed publish uses `--no-restore`
and does not repeat it. The final `bundle-manifest.toml` and `SHA256SUMS`
bind every payload and deployment asset. The installer verifies before and
after copy, uses a whole-bundle release identity, and leaves the prior
`current` release active if any uncommitted stage fails. Real and `DESTDIR`
installs take the same bounded target-level lock before bundle validation;
staging roots that are `/` or symbolic links fail closed.

Run the physical Linux bundle, upgrade, rollback, bridge, and live-health smoke
after packaging:

```bash
scripts/validation/leserpent_linux_bundle_smoke.sh artifacts/leserpent/linux-x64
```

### I want to package the Leserpent macOS app

Build a locked NativeAOT app, embed the native `leserpentd`, create and strictly
verify a local ad-hoc signature, and atomically publish the bundle with:

```bash
cargo dev package desktop
```

Build and install the same artifact through the versioned user-local installer:

```bash
cargo dev deploy desktop
cargo dev deploy desktop --launch
```

Both commands default to
`artifacts/leserpent-avalonia/Leserpent.app`. Pass `--output APP` for another
artifact path or `--silvortex-issuer HTTPS_ORIGIN/` for the reviewed public
account issuer. Concurrent desktop pipelines are rejected by a managed lock;
failed pending bundles are removed and an existing complete artifact remains
available. Automatic replacement is limited to the default managed artifact;
an existing custom `--output` must be moved explicitly before packaging.

After a Developer ID Application certificate and a `notarytool` Keychain
profile are provisioned, produce the formal Apple artifact through the same
atomic entrypoint:

```bash
cargo dev package desktop \
  --identity 'Developer ID Application: ORGANIZATION (TEAMID)' \
  --notary-profile leserpent-notary
```

The two options are inseparable. Formal mode runs a strict readiness preflight,
signs every nested native payload and the app with Hardened Runtime and a secure
timestamp, waits for explicit notarization acceptance, staples and validates
the ticket, and finishes with Gatekeeper. All work occurs on a pending bundle;
failure leaves the previously published app untouched.

For lower-level bundle diagnostics after publishing an `osx-arm64` directory,
run:

```bash
cargo build --release -p leserpentd --features native-ssh
cargo run --bin gewyvern_leserpent_bundle -- \
  --publish-dir artifacts/leserpent-avalonia/osx-arm64 \
  --daemon target/release/leserpentd \
  --output artifacts/leserpent-avalonia/Leserpent.app
```

This native entrypoint validates a flat, symlink-free publish directory,
excludes external debug symbols, requires an arm64 Mach-O `leserpentd`, copies
the Avalonia executable, Rust daemon, and dylibs, and writes stable bundle
metadata plus the checked `.icns`. Existing output is never replaced
implicitly. With `--version` omitted, the official bundle inherits the root Rust workspace version
for both plist version fields; the option remains
available only for deliberate downstream overrides. Signing and notarization
consume this bundle in a later release step.

Install the checked app for the current user through the native versioned
installer:

```bash
cargo run --bin gewyvern_leserpent_install -- install \
  --app artifacts/leserpent-avalonia/Leserpent.app
cargo run --bin gewyvern_leserpent_install -- status
cargo run --bin gewyvern_leserpent_install -- rollback
```

The default launcher is `~/Applications/Leserpent.app`; immutable releases and
the bounded `current`/`previous` metadata live below
`~/Library/Application Support/Leserpent/Installer`. The Rust implementation
copies a bounded symlink-free bundle, validates arm64 thin or universal Mach-O
payloads and exact plist identity, requires executable main and `leserpentd`
payloads, and binds every native payload into the immutable release ID. It
strips group/world write permissions and atomically replaces managed links.
Install and rollback hold a bounded exclusive lock on the installer root, while
status queries use a shared lock and cannot observe a half-switched link pair.
It rejects an existing unmanaged launcher
instead of overwriting it. Profiles, managed CAs, caches, Keychain credentials,
and Orchestra data remain outside the installer root and survive upgrades and
rollback. `--root` and `--launcher` accept absolute paths for packaging proof or
managed deployments.

The retained physical-host fixture is
`docs/fixtures/leserpent_macos_install_rollback.json`. It proves a signed local
`1.4.0 -> 1.4.1 -> 1.4.0` cycle and launches a real control fixture through the
rolled-back stable app link. It explicitly records `signature=adhoc`, no Team
ID, and `apple_release_evidence=false`; Developer ID signing and notarization
remain a separate release gate.

### I want to sign and notarize the Leserpent macOS app

Prefer the unified `cargo dev package desktop --identity ...
--notary-profile ...` command above. Use the native release entrypoint directly
only to diagnose or rerun an individual stage after bundle creation:

```bash
xcrun notarytool store-credentials leserpent-notary

cargo run --bin gewyvern_leserpent_release -- preflight \
  --app artifacts/leserpent-avalonia/Leserpent.app \
  --keychain-profile leserpent-notary \
  --require-ready

cargo run --bin gewyvern_leserpent_release -- sign \
  --app artifacts/leserpent-avalonia/Leserpent.app \
  --identity 'Developer ID Application: ORGANIZATION (TEAMID)'

cargo run --bin gewyvern_leserpent_release -- notarize \
  --app artifacts/leserpent-avalonia/Leserpent.app \
  --keychain-profile leserpent-notary
```

Before any account-enabled release proof, verify that all current product
capabilities remain MIT-licensed, open source, free, and available without Team
Silvortex login, and that only a future hosted-service marker may require an
explicit entitlement:

```bash
artifacts/leserpent-avalonia/Leserpent.app/Contents/MacOS/Leserpent.Avalonia \
  --verify-product-access-policy
```

This check must report `core_license=MIT`, `core_price=free`, and
`current_core_paywall=false`. It is offline, does not replace endpoint-bound
daemon credentials, and does not contact the account provider. After 2.0 and
before the first future hosted subscription service, generate the production
account proof from the final signed/notarized packaged executable, then verify
the bounded result without trusting the managed producer:

```bash
artifacts/leserpent-avalonia/Leserpent.app/Contents/MacOS/Leserpent.Avalonia \
  --prove-silvortex-account "${TMPDIR:?}/leserpent-account-proof.json"

cargo run --bin gewyvern_leserpent_release -- account-proof \
  --app artifacts/leserpent-avalonia/Leserpent.app \
  --evidence "${TMPDIR:?}/leserpent-account-proof.json"
```

The v2 proof contains no issuer, identity, credential, credential digest, or
daemon authority. It binds the executable and the exact `Info.plist` bytes
used for account configuration. The strict Rust verifier rejects unknown or
duplicate fields, unsafe boundary flags, and any binary or plist drift. If a
bound file changes, rerun the real browser/vault workflow and verify the new
evidence against the final bundle.

`preflight` emits one JSON object by default, including when readiness is
blocked. Add `--require-ready` when it is a mutation gate; blocked readiness
then exits nonzero and includes the report in the bounded error. Its v2 schema
binds separate SHA-256 digests for the main executable and bundled
`leserpentd`, validates the checked entitlements, inventories `codesign`,
`ditto`, `plutil`, `security`, `spctl`, `xcrun`, `notarytool`, and `stapler`, and
counts valid Developer ID Application identities. Add `--keychain-profile
leserpent-notary` after storing credentials to verify that profile through
`notarytool history` without exposing its secret.
Only a complete toolchain, at least one valid identity, and a validated profile
produce `release_ready=true`. The historical fixture at
`docs/fixtures/leserpent_macos_release_preflight.json` is explicitly blocked by
the absent identity and unrequested profile; it records an earlier readiness
observation, not a current-artifact or Apple release claim. Its version and
binary hashes are not rewritten during release alignment.

The credentials command is intentionally interactive. The release CLI has no
Apple ID, password, API private-key, or Team ID input surface. It requires a
timestamped Developer ID signature and Hardened Runtime, waits for an explicit
notary `Accepted` result, staples and validates the ticket, and runs Gatekeeper.
Use `verify --allow-adhoc` only to test local signature structure. Its output
sets `runtime_launch=false`: ad-hoc signatures have no Team ID and therefore
cannot establish the same-team library-validation relationship required by a
Hardened Runtime app with separately signed `leserpentd` and dylibs. Ordinary
ad-hoc bundles are used for local UI smoke; release launch proof requires
Developer ID.

For a packaged saved-profile/Keychain proof, create a bounded profile and CA
under the current user's `$TMPDIR` with an unused high loopback port, then run:

```bash
artifacts/leserpent-avalonia/Leserpent.app/Contents/MacOS/Leserpent.Avalonia \
  --verify-packaged-profile-startup "$TMPDIR/leserpent-proof/profile.json"
```

The executable refuses shared `/tmp`, non-loopback or low-port origins,
out-of-root CA files, symlinked directories, and pre-existing credentials. It
generates the non-secret fixture token internally and always removes a
successfully created Keychain item before reporting success.

To verify desktop connection switching and forgetting without touching the
real credential store, run:

```bash
dotnet run --project apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj -- \
  --verify-connection-maintenance
dotnet run --project apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj -- \
  --verify-connection-management-controls
```

The first command uses an injected in-memory vault to prove endpoint-scoped
deletion, profile cleanup, and stale-profile rejection. The second opens the
real Avalonia settings surface and audits both settings and destructive
confirmation controls.

To verify private desktop CA import and migration semantics, run:

```bash
dotnet run --project apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj -- \
  --verify-desktop-ca-store
```

The probe generates local CA and leaf fixtures, then checks strict single-PEM
parsing, CA/key-usage policy, SHA-256 addressing, atomic private permissions,
idempotency, managed-file replacement and symlink rejection, bounded pruning,
and stale temporary-file cleanup. It never touches the product trust directory.

### I want to prove Leserpent local transport compatibility

Run:

```bash
cargo run --quiet --bin gewyvern_validate -- leserpent-transport
```

The named shelf runs eight layers: canonical wire-v1 protocol tests, legacy-v1
adaptation fixtures, CLI/Leselang command and query parity, a real native CLI to
daemon Unix-socket roundtrip, the daemon IPC security boundary, and the
authenticated HTTPS wire boundary, WebSocket event security, and a native
CLI-to-daemon HTTPS vertical path. The remote layers prove a real TLS
roundtrip, bearer-token rejection, strict bounded HTTP framing, private-key
file safety, shared wire-v1 dispatch, explicit CA/hostname verification, and
remote command/query/watch parity. They also prove required WebSocket
subprotocol negotiation, endpoint-redacted revision snapshots, and cursor
resynchronization. Each layer has an independent log under
`target/validation/leserpent-transport/`, alongside
`transport-summary.json` and `evidence-index.json`.

This proof deliberately excludes Windows named pipes, remote GUI, and mobile
clients. Their absence is recorded in the summary rather than reported as a
cross-platform success. The current macOS arm64 and physical Linux x86_64 runs
produce matching eight-suite, 28-test, 41-invariant summaries.

### I want to inspect Leserpent v1 schema freeze readiness

Run:

```bash
cargo run --quiet --bin gewyvern_validate -- leserpent-schema-freeze
```

The bounded inventory at
`project/release/leserpent-v1-schema-inventory.json` names the command, query,
effect-plan, UI IR, and wire v1 contract sources. It records source anchors and
fixed proof IDs, but cannot supply executable Cargo arguments. The native shelf
rejects path traversal, symlinks, oversized sources, duplicate families or IDs,
unknown proof IDs, missing anchors, non-v1 contracts, and state mismatches.
The companion
`project/release/leserpent-v1-compatibility-baseline.json` pins byte lengths and
SHA-256 fingerprints for five canonical/legacy wire fixtures and four renderer
fixtures. An unreviewed fixture change therefore fails before semantic proof
execution; an intentional compatibility change must update the fixture and its
reviewed candidate baseline together.

The frozen `project/release/leserpent-2-scope-freeze.json` separately fixes the
10 core capability families, 10 permitted closure-work categories, 10
explicit deferrals, both authority-document anchors, and their live status-cell
references. Scope expansion, a missing deferral, an Etragon/1.x authority leak,
or a stale status reference fails closed. This manifest contains no executable
arguments and reports `scope_freeze_ready=true` independently of the still
candidate version-1 schema inventory.

The companion `project/release/leserpent-2-patch-seal.json` protocolizes the
ten contiguous `1.20.0` through `1.20.9` closure slots before `2.0.0`. The same
native shelf rejects reordered or missing versions, duplicate focus IDs, work
outside the scope-freeze whitelist, incomplete closure-family coverage, and a
product version outside the frozen release window. The retained summary embeds
the validated plan, current inferred slot, and exact slot count.

Five fixed Rust suites plus one locked, filtered xUnit suite require at least 65
non-vacuous tests and retain the actual observed count in their logs,
`schema-freeze-summary.json`, and
`evidence-index.json` under
`target/validation/leserpent-schema-freeze/`. The inventory deliberately stays
`candidate` and the summary stays `freeze_ready=false` until every Gate 7
security, performance, migration, packaging, rollback, and platform-release
criterion has reproducible evidence.
The same summary records all 11 compatibility fingerprints, keeping exact
machine-format drift separate from the semantic proof suites. It also embeds
the validated scope manifest and its three bounded counts so retained evidence
shows exactly what 2.0 did and did not promise.
The migration subset replays runtime journal v1, v3 snapshot, and complete v6
state into the current v10 journal, rejects inconsistent migration history, and
proves legacy runtime-list, status-refresh, and error adaptation. Each migration
suite has a fixed minimum of four observed tests, so an accidental empty Cargo
filter cannot appear as release evidence.
The managed subset runs exactly the security project's
`SqliteOrchestraRunStoreTests` through the shared locked-restore Dotnet proof
runner. Its ten observed tests cover SQLite v1 in-place migration, legacy
JSON-to-SQLite import, request identity, delete cascades, bounded retention,
serialized durable saves, and failed-save snapshot preservation. Temporary
Dotnet artifacts are isolated and removed after both success and failure.
Failure injection also forces a unique-index violation after transactional
replacement starts, proving the prior SQLite rows survive and a corrected retry
succeeds. A separate startup failure proves the legacy JSON remains byte-for-byte
unchanged, then recovers through both SQLite retry and explicit JSON-only operator
rollback.

The Linux package proof runs
`scripts/validation/leserpent_linux_bundle_smoke.sh` against a target-built
NativeAOT bundle. It creates two immutable staged releases, validates atomic
`current` and retained `previous` links, invokes explicit rollback, proves
configuration and state remain external to releases, executes the Rust bridge,
and starts the rolled-back service through its live health endpoint. Its JSON
evidence is retained under
`target/validation/leserpent-linux-bundle-smoke/`; the checked shell entrypoint
remains the package fixture while release orchestration stays in the native
validation harness.

### I want to prove registration lost-response recovery

Run:

```bash
scripts/validation/leserpent_registration_recovery.sh
```

The entrypoint builds the production Rust `leserpentd`, locked-restores and
builds the .NET compatibility projects, then runs the real Unix-socket recovery
campaign. An owner-private proxy drops two registration responses after daemon
commit; the first compatibility process is force-killed and a fresh process
must replay the same command, reuse persisted discovery without HTTP requests,
apply discovery intake once, bind only the fresh local credential, and clear
schema-v9 pending state. Evidence is written under `target/validation/` with a
platform-specific name. The retained physical Linux x86_64 result is
`docs/fixtures/leserpent_registration_recovery_linux_x86_64_20260825.json`.

Use the trusted host wrapper to repeat the same proof remotely:

```bash
scripts/remote/run_on_linux_host.sh -- scripts/validation/leserpent_registration_recovery.sh
```

### I want to prove command parity and restart recovery

Run:

```bash
cargo run --quiet --bin gewyvern_validate -- leserpent-parity-recovery
```

The command runs thirteen suites over the current migrated command surface:
frontend-neutral lowering, authorization/confirmation/idempotency,
CLI/Leselang parity, VM continuation and journal re-entry, and runtime SQLite
recovery injection, plus the non-vacuous .NET control-plane security suite,
authenticated remote wire, native CLI parity, the
Avalonia remote-state conformance runner, and a real Rust daemon to .NET
WebSocket plus HTTPS mutation vertical, and the mobile lifecycle conformance
runner. It covers
snapshot corruption and prior-generation
fallback, expired-lease redelivery, stale-worker fencing, final-attempt worker
crash handling, status projection replay, and refresh outbox repair.

Evidence is retained under
`target/validation/leserpent-parity-recovery/`. The summary records
actual test counts and the validator fails if any suite runs fewer than its
declared minimum, including a zero-test filter mistake.
The current macOS arm64 and physical Linux x86_64 checkpoints both prove 13
suites, 231 observed tests, and 155 invariants against a minimum of 206 tests.
The Linux summary records kernel `6.17.0-35-generic`, Rust/Cargo `1.95.0`, and
.NET `10.0.109`. Every summary now records bounded kernel and toolchain
provenance, and stale summary/index files are removed before a new run starts.

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
cargo run --quiet --bin gewyvern_validate -- --json release-gate --leserpent-proof
cargo run --quiet --bin gewyvern_validate -- --json release-gate --remote-host-validation
```

The `extra` object for `release-gate` currently exposes:

- `stages.build_packages`
- `stages.release_container_check`
- `stages.three_module_stack_smoke`
- `stages.pathological_container_validation`
- `stages.leserpent_parity_recovery`
- `stages.leserpent_schema_freeze`
- `stages.remote_linux_host_validation`
- top-level `ship_signal = "timing_watch"` when the remote host passed but one
  of the soft timing budgets regressed
- `remote`

`stages.leserpent_parity_recovery` and `stages.leserpent_schema_freeze` are both
true only when the caller explicitly selects `--leserpent-proof` and both
shelves succeed. This keeps the Gewyvern-only gate independent while giving
combined releases one machine-readable result and one artifact index.

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

The artifact index uses schema v2. JSON and text carry the same bounded
`publication_id`; synced staged writes publish the text first and JSON last as
the machine commit point. Missing or mismatched IDs are a torn publication and
must fail closed.

When the remote Linux stage covers the Leserpent control-plane and the paired
Local Orchestra/saved-daemon language-pack NativeAOT proofs, the index includes dedicated
`remote_leserpent_control_plane_aot` and
`remote_leserpent_language_pack_local_orchestra_aot` high-signal artifacts.
Their `present` status is tied to the current release-gate checks after strict
local revalidation; an older directory on disk cannot promote a skipped stage.

Use those two companion files as the compact directory-level index of which
release-facing evidence shelves are currently present under `target/validation/`,
including the optional Leserpent parity/recovery and `juice-shop-container`
shelves when they exist.
Stage-owned entries report `not_run` when the current invocation skipped that
stage, even if an older evidence directory is still present. Companion shelves
without a release-gate stage continue to report their actual path presence.

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

`resilience-roundtrip` prepares helper programs, configuration and a runbook;
it does not start a runtime or execute failure/recovery scenarios. A successful
report from that command is preparation evidence only. Live malformed-input
recovery is exercised by `runtime-operator` and `runtime-lifecycle`; use the
fault-injection runbook for the broader external-engine failure campaign.

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
sudo env GEWY_CONTAINER_TC_DEVICE=gwlab0 cargo run --quiet --bin gewyvern_validate -- juice-shop-container-validation
```

Or through the server-aware wrapper:

```bash
GEWY_CONTAINER_TC_DEVICE=gwlab0 bash scripts/validation/juice_shop_container_validation.sh
```

Use this when the real question is:

- can `gewyvern` preserve suspicious target-side evidence from a live Docker lab?
- can the same Linux host still prove tracepoint, kprobe, and tc attach health?
- do we have one repeatable practical-target shelf that is stronger than a synthetic demo?

If you want the same style of proof for protocol/authentication denial instead
of HTTP error evidence, run:

```bash
GEWY_CONTAINER_TC_DEVICE=gwlab0 bash scripts/validation/ftp_denied_container_validation.sh
GEWY_CONTAINER_TC_DEVICE=gwlab0 bash scripts/validation/ldap_bind_denied_container_validation.sh
```

These practical suites also contain same-host eBPF attach proof. Docker group
access is automatic on the validation account, but BPF privilege remains a
separate explicit requirement and is never silently elevated by the wrapper.

Create a dedicated lab interface before running these suites and pass its name
through `GEWY_CONTAINER_TC_DEVICE` (`gwlab0` above is an example, not auto-created).
The suites validate this target before starting containers and do not select a
default-route interface. Remove only the interface created for your run afterward.

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

### I want real kernel events reconstructed and replayed

Build on the Linux server, then run the native validator on the host:

```bash
cargo build --locked --bin gewyvern_validate
sudo target/debug/gewyvern_validate --json linux-tcp-capture --out-dir target/validation/tcp-capture-new
```

The output directory must be new. This requires kernel BTF, a mounted writable
`/sys/fs/bpf`, Clang's BPF backend, libbpf development headers, `bpftool` with
`autoattach` and BTF-formatted map dumps, and BPF attachment privileges. Compiling
in a resource-bounded Linux build container is fine; run the actual capture on
the authorized host. Do not start a local Docker build on a disk-constrained Mac.

By default the Rust driver owns two IPv4 loopback sockets and selects their real `SO_COOKIE`
values before connecting. Its CO-RE tracepoint program records at most 128 TCP
state events, not payloads, PIDs or unrelated sockets. A real HTTP exchange,
half-close, refused connection and unselected decoy exercise the chain. The
validator checks exact cookie/tuple/netns identity, loss counters, lifecycle and
facts/flows/reasons after export replay. It removes only its own BPF pins; it
does not install TC rules or touch the default-route interface.

Read `summary.json`, `*-raw.json`, `*-export.json` and `tools.log`. Raw captures
retain monotonic nanoseconds; runtime export timestamps use milliseconds.
Exports remain `unverified-local`. The client's HTTP status is not an inferred
Gewyvern classification. This is a bounded validation backend, not the general
live CLI, payload decoder, PID attribution or sustained-load acceptance test.
See the [physical-host evidence](fixtures/gewyvern_tcp_capture_linux_x86_64_20261104.json).

To correlate real container requests with those kernel facts, use the fixed lab mode:

```bash
sudo target/debug/gewyvern_validate --json linux-tcp-capture --juice-shop --out-dir target/validation/container-capture-new
```

This uses the already-cached Juice Shop digest recorded in the
[container capture proof](fixtures/gewyvern_container_capture_linux_x86_64_20261104.json).
It never pulls images automatically and does not accept an arbitrary target URL.
The validator uses the local Docker socket, creates a labelled internal bridge
and resource-bounded target container, and publishes no ports. It binds its owned
client sockets to that bridge's gateway and connects directly to the inspected
private container IP. Both addresses and the two socket cookies are pinned into
the BPF filter. This host-to-internal-bridge access follows Docker's
[internal bridge model](https://docs.docker.com/engine/network/port-publishing/#gateway-modes);
it is not a host-network container or a change to existing network policies.

`correlation.json` joins each native-client request/response to exact socket
cookie, tuple, network namespace, monotonic time bounds, continuous TCP states,
export fact IDs and flow ID. The file guard (403) and SQL error (500) are checked
against their response bodies. A third, unselected normal request (200) must
produce no captured facts. Raw `*-request.http` and `*-response.http` files have
SHA-256 hashes in the correlation record; `final-export.json` contains TCP state
facts only. The bounded client accepts complete Content-Length or simple chunked
framing, rejects conflicting/truncated data, and does not decompress bodies or
follow redirects. Response size is limited to 256 KiB with a five-second read
deadline. Chunk extensions/trailers are deliberately unsupported by this fixture.

The container mode additionally requires a writable unified cgroup v2 hierarchy
and BPF cgroup link support. It moves only the validation process into a new
run-owned cgroup before creating the selected sockets. An ingress-only BPF
program filters their exact cookies, private addresses and ports, always allows
traffic, and snapshots at most 128 IPv4/TCP events: SYN-ACK origins plus nonempty
payloads with a 64-byte prefix each. It preserves actual IP/TCP lengths, flags, sequence, acknowledgement and
window fields. The socket network namespace is joined from captured TCP facts,
not inferred from a process name. These are kernel skb snapshots, not a promise
of individual on-wire frames or TCP stream reassembly.

`packet-prefix-raw.json`, `packet-events.json` and `packet-export.json` keep this
evidence separate from native-client I/O. The validation-only
`ebpf/validation/http_prefix.gewy` rules emit **packet-local candidates**, not
verified HTTP response statuses. Facts, flows, reasons, program flows and protocol
IR replay unchanged. `response-prefixes.json` separately reconstructs only the
first 13 bytes relative to a captured peer SYN-ACK sequence number, with anchor
and contributing payload fact IDs. It handles splits, byte-identical duplicates,
out-of-order prefix pieces and 32-bit sequence wrap; missing origins, gaps or
conflicting bytes stay explicitly unverified. A later body packet cannot become
the response start. No reconstructed bytes are inserted as a synthetic packet.
This derived result must also replay and match the independently saved client
prefix. The container summary is now schema 2: `packet_dsl_status_candidates`
and `verified_http_statuses` replace the ambiguous `kernel_dsl_http_statuses`.
Any selected-packet parse failure, overflow or reported map loss fails the run.
No full body, later/interim response parsing, TLS plaintext or vulnerability finding
is claimed; payload-bearing SYN-ACK/TCP Fast Open is explicitly unsupported.

To reproduce a split 403 response and a normal 200 response whose **body** starts
with a fake 500 line, use the separate, Docker-free loopback scenario:

```bash
sudo target/debug/gewyvern_validate --json linux-tcp-capture --http-boundaries --out-dir target/validation/http-boundaries-new
```

This requires the same BPF/cgroup prerequisites. It binds a fresh loopback port,
reads each response piece before sending the next, and checks the actual BPF
events rather than assuming write boundaries equal packet boundaries. The raw
DSL must expose the body candidate; the origin-checked result must remain 200,
while the split response becomes 403. `--juice-shop`, `--http-boundaries`,
`--auth-breadth`, `--capture-recovery`, `--capture-load`, `--capture-stream`, `--capture-runtime`, `--capture-mixed`, `--capture-soak`, `--capture-soak-hour` and `--capture-memory-control` are mutually exclusive and reject mixed/duplicate flags
before creating resources.

To cross-check existing packages against real text/binary authentication protocols:

```bash
sudo target/debug/gewyvern_validate --json linux-tcp-capture --auth-breadth --out-dir target/validation/auth-breadth-new
```

This native Rust entrypoint requires the cached digest-pinned `fauria/vsftpd`
and `osixia/openldap` images named in `auth_breadth.rs`. It never pulls images,
publishes ports or uses host credentials. Each target gets a new internal Docker
network, a one-core/512-MiB/128-process limit and only fixture-required capabilities.
Owned containers, anonymous volumes and networks are removed by identity; no
global Docker cleanup occurs. Mac does not need either image.

For each target, selected success and denial connections are compared with an
unselected denied connection to the same server. The client parses bounded
single-line [FTP replies](https://www.rfc-editor.org/rfc/rfc959.html) or framed
[LDAP simple-bind results](https://www.rfc-editor.org/rfc/rfc4511.html).
Independently captured server bytes must cover a contiguous range from a real
SYN-ACK and exactly match the client response. Gaps, conflicting bytes, truncated
packet prefixes, unsupported framing and the 4-KiB stream bound fail closed.
The client has an absolute exchange deadline and never exports request passwords.

The same facts exercise `ftp/denied`, `ftp/session`, `ldap/bind-denied` and
`ldap/bind` without changing their rule sources. The generic LDAP bind package
means a response was observed, not authentication succeeded; the framed result
code supplies that distinction. Every matching stage must reference captured
packet evidence. JSON replay preserves facts, flows, reasons, stages and IR.
Declared route/process probes not collected by this fixture are marked unavailable,
not successful attachments. Requests are not captured and process IDs are not
inferred. After both targets, fresh HTTP capture/replay must pass again.

This is bounded plaintext authentication diagnosis, not vulnerability detection,
arbitrary protocol identification, generic TCP reassembly, FTP data transfer,
LDAP search/modify/SASL, TLS decryption, IPv6 or UDP coverage.

To inject bounded collection faults and verify recovery, without Docker:

```bash
sudo target/debug/gewyvern_validate --json linux-tcp-capture --capture-recovery --out-dir target/validation/capture-recovery-new
```

This accepts exactly 128 observed packet events, rejects packet/TCP event-budget
overflow, forces real map-write loss using a two-entry test map, and deletes one
owned snapshot entry after detachment to test an unreported gap. It never drops
or changes network packets. Read each case's `result.json`, `packet-rejection.json`
or `tcp-rejection.json`; rejection codes distinguish `capacity_exceeded`,
`reported_loss` and `snapshot_incomplete`. Rejected packet cases cannot publish
`packet-export.json`. Independent, complete TCP evidence can still be retained.
Top-level `ok:true` means the fault campaign passed, not that rejected captures
became valid evidence.

The same native binary supervises its own child workers, waits for an ownership
manifest, then exercises error return, SIGABRT and SIGKILL. Both BPF link IDs must
disappear before residual pin cleanup. Directory device/inode identities and
the exact waited child constrain cleanup; no prefix-wide scan deletes resources.
Core dumps are disabled only in these disposable workers. Each exit and the
collection-loss sequence are followed by a fresh capture and deterministic replay.
Do not rebuild the running validator while a campaign is active.

See the [physical-host recovery proof](fixtures/gewyvern_capture_recovery_linux_x86_64_20261104.json).
This is not sustained-load/ring-buffer acceptance, a test of killing the supervisor,
a host-reboot test or coverage of the pre-ready autoattach-to-fd handoff window.

To measure sustained **capacity-exhausted** behavior against a no-capture baseline:

```bash
cargo --config profile.dev.package.gewyvern.opt-level=2 build --locked --bin gewyvern_validate
sudo target/debug/gewyvern_validate --json linux-tcp-capture --capture-load --out-dir target/validation/capture-load-new
```

This runs baseline/capture/capture/baseline phases, each with two seconds of
warmup and sixty measured seconds. Two selected loopback clients each keep one
64-byte request/echo response in flight. All bytes and request sequences are
checked. Only a fresh owned cgroup is limited to two CPU cores of quota, 512 MiB
and 32 tasks; missing delegated controllers are an error, not a reason to change
global controller settings. Do not run builds or other benchmarks concurrently.
The build command optimizes the project package, not every dependency and not
the entire release profile; record the exact build command with results.

Read per-phase `result.json` and the top-level `summary.json`. CPU is the measured
test-cgroup delta, including clients, echo servers and monitoring. Some kernel
work may be charged outside that cgroup, so this is not isolated BPF CPU cost.
RSS and cgroup memory are sampled at roughly one-second intervals; they include
fixed-size latency histograms and kernel/cache charges, not just collector state.
RTT percentiles use one-microsecond upper-bound buckets; values at or above
100 ms go into an explicit overflow bucket. A null percentile is not zero.
Maximum and mean latency are retained independently of histogram overflow.

The existing sampler has **128 retained slots and no continuous consumer**. It
fills during warmup, so the measured capture phase exercises parsing/counting
and budget rejection, not sustained export/ingest. `measured_rejection_percent`
is therefore expected to be 100%; it must not be advertised as network packet
loss or as a production streaming loss rate. Capacity accounting, no incomplete
exports, byte integrity and a fresh split-prefix capture/replay after pressure
are checked. This is a closed-loop pressure test, not an open-loop offered-load
tail-latency bound, wire packet-rate measurement, long-duration leak proof or
lossless production capture benchmark. Failed phases do not publish a successful
top-level summary. Original phase results remain available if a later phase fails.

To exercise bounded continuous kernel-to-Rust delivery instead of that snapshot
control, use the same optimized build and a new output directory:

```bash
sudo target/debug/gewyvern_validate --json linux-tcp-capture --capture-stream --out-dir target/validation/capture-stream-new
```

This mode requires `libbpf.so.1` with `ring_buffer__consume_n` on the Linux host.
It is loaded only for the native reader; ordinary builds and unit tests do not
link against a system libbpf installation. The same packet parser emits 128-byte
records to a fixed 1-MiB BPF ring. A thread-confined Rust callback consumes at
most 512 records per batch, with bounded waits and cancellation checks. ABI size
and prefix bounds are checked before callback delivery; callback errors and Rust
unwinding are contained at the FFI boundary. No unbounded user-space queue or
full capture file is created. The kernel never waits for the consumer: output
failure counts a rejected new event without modifying network traffic or
overwriting unread evidence. See the [kernel ring-buffer semantics](https://kernel.org/doc/html/latest/bpf/ringbuf.html)
and [libbpf reader API](https://libbpf.readthedocs.io/en/latest/api.html).

The campaign runs a sixty-second baseline, a sixty-second continuous phase, a
ten-second phase with the consumer paused for roughly two seconds, another
sixty-second continuous phase, and a final sixty-second baseline. Each phase has
two seconds of warmup. CPU/memory/task limits and owned traffic match the load
control. Warmup is drained to an explicit checkpoint. After traffic stops, the
link detaches before the reader drains the remaining ring and reconciles
`received + dropped == total`; malformed input and unexplained sequence gaps
fail the run. A paused case must drop events and then deliver more than one
ring's capacity, demonstrating actual resumed collection, not just draining the
old buffer. A subsequent split-prefix capture/replay checks the snapshot path.

`observation_complete` is false when a phase lost events; `normal_windows_complete`
records whether both unpaused phases were complete. Top-level `ok` means the
accounting/recovery campaign passed, not that the intentional pause was lossless.
Delivery latency is kernel monotonic timestamp to checked Rust sink acceptance,
separate from request RTT. Events older than 100 ms stay visible in the histogram
overflow count and exact maximum; a null p99 is not zero. The sink only checks
owned echo bytes/sequence and records bounded metrics/samples. This is not yet
full `RuntimeSession` ledger ingest, persistent storage, general TCP reassembly
or lossless capture on arbitrary networks. CPU and memory remain whole-test
measurements; no collector-only or open-loop latency claim is made.

To include actual runtime ledger ingestion and analysis, use:

```bash
sudo target/debug/gewyvern_validate --json linux-tcp-capture --capture-runtime --out-dir target/validation/capture-runtime-new
```

The same five-phase campaign now passes every delivered packet through
`RuntimeSession::ingest` and `into_export_bundle_with_context`, including flow, program and
reason analysis. Independent sessions contain at most 256 facts; completed
bundles are checked then released, with no unbounded user-space queue or export
archive. Fact IDs are monotonic across batches, while session IDs identify the
independent batches. There is no synthetic TCP-state or process evidence.
Warmup's partial batch is flushed before measurement starts. Reports reconcile
ingested/analyzed facts with consumer receipts and include the batch high-water
mark, analyzed batch count and kernel-to-analysis-completion latency. This last
metric includes waiting to fill a batch and analysis but excludes sampled replay.

One caller-owned `RuntimeExportContext` per capture phase fixes protocol metadata
for its independent batches and sampled reanalysis, rather than rescanning the
protocol directory on every export. `stage_profile` records bounded aggregates
for one-time catalog discovery, conversion, ingestion, rotation, analysis,
validation, sampled replay and release. These include consumer-thread wall time
and paired `CLOCK_THREAD_CPUTIME_ID` samples: every batch stage and every 257th
conversion/ingestion call. Setup/warmup and timer overhead remain included;
wall-minus-CPU is not a quota-only wait counter, and sampled totals are not
extrapolated. No per-event timing history is retained. Existing export callers are unchanged;
they must opt into an explicit context to reuse a catalog snapshot.

For repeatable hotspot diagnosis without BPF or live traffic, run on Linux:

```bash
cargo --config profile.test.package.gewyvern.opt-level=2 test --locked -p gewyvern --lib profile_runtime_sink_stages -- --ignored --nocapture
```

This ignored fixture feeds 32,768 synthetic two-cookie records through
the same adapter. Its fixture elapsed time excludes sink setup; one-time catalog
discovery is reported separately. It is not a kernel throughput or loss proof.

Representative batches also undergo JSON roundtrip and fresh-session reanalysis
using the same native capture adapter and rules. This does not use generic
`ExportBundle::replay`, which assumes builtin probe dependencies and retains
serialized program outputs. Reanalysis compares recomputed facts, flows,
reasons, program stages, protocol IR and findings. No packet export is published,
even when a phase is complete; the reports are bounded diagnostic evidence.

`ok` still means accounting and recovery passed, not zero loss. A slow runtime
may overflow the ring even without the deliberate pause, and such phases remain
explicitly incomplete. Runtime batch analysis is not a continuous stateful
session: TCP context does not cross batch boundaries. Full-stream serialization,
durable storage, generic-adapter offline replay and long-duration memory behavior
remain separate acceptance work.

For bounded same-process memory retention diagnosis on glibc Linux, use:

```bash
sudo target/debug/gewyvern_validate --json linux-tcp-capture --capture-soak --out-dir target/validation/capture-soak-new
```

Three 300-second runtime capture windows use the same limits and independent
batches, each followed by full teardown and 30 seconds idle. Every window writes
one-second RSS/cgroup samples and roughly thirty-second allocator snapshots to
private JSONL journals, using two fixed 8-KiB buffers and scalar first/last/peak values.
No historical sample trees or cycle reports survive at the clean checkpoints.
The final summary is reconstructed from small disk reports only after measurement.
Eight observer-only checkpoints provide a no-traffic control for probe overhead.
Post-teardown thread and file-descriptor counts must return to baseline. The
final fresh HTTP capture/replay still has to pass. `ok` means the diagnostic
campaign completed, not that all events were delivered or memory is leak-free.

The [glibc allocator snapshot](https://man7.org/linux/man-pages/man3/malloc_info.3.html)
covers all arenas. Nonfree space is an upper-bound diagnostic including allocator
metadata, thread caches and measurement overhead, not exact live Rust objects.
After all three normal windows and idle observations, one
[diagnostic trim](https://man7.org/linux/man-pages/man3/malloc_trim.3.html)
tests whether free pages can be returned. No trim occurs during load, no global
allocator settings change, and this does not add trimming to production code.
Compare both post-teardown nonfree growth and reclaimable RSS; RSS alone cannot
distinguish a leak from allocator retention. XML snapshots are created exclusively
with private permissions; unsupported/ambiguous totals fail instead of becoming
zero. A fifteen-minute single-host run is not an indefinite soak guarantee.

For one hour of load (three twenty-minute windows plus idle/restart), build the
optional allocation witness and use the distinct hour mode:

```bash
cargo --config profile.dev.package.gewyvern.opt-level=2 build --locked --features capture-allocation-audit --bin gewyvern_validate
sudo target/debug/gewyvern_validate --json linux-tcp-capture --capture-soak-hour --out-dir target/validation/capture-soak-hour-new
```

The feature installs a forwarding `System` allocator only in `gewyvern_validate`,
not in the library or services. Two process-wide atomic counters track successful
Rust allocations/reallocations and their frees, including cross-thread ownership.
The allocator callback never allocates or keeps pointer maps. Snapshots contain
requested live Rust bytes/blocks, not C allocations, allocator cache sizes or RSS.
During load the pair is not an atomic census; compare quiescent clean checkpoints.
The hour mode rejects an inactive witness before creating the output directory.
Normal builds keep the default allocator. The diagnostic counters add overhead,
so audited throughput must not be presented as an ordinary-build benchmark.
Historical schema-1 evidence retains its original sample/report retention;
schema 2 explicitly records journals, observer controls and clean checkpoints.

After the traffic campaign, the same audited binary can isolate observer/cache
behavior without network activity or BPF privileges:

```bash
target/debug/gewyvern_validate --json linux-tcp-capture --capture-memory-control --out-dir target/validation/capture-memory-control-new
```

This runs 256 report-free observations on the main thread and another 256 on a
temporary worker, then joins the worker before the final snapshots and trim.
Compare points within the same thread series: the worker includes a waiting
parent's bookkeeping. Results are assembled after measurement, and no full
series stays resident during the observations. Main-thread caches can remain
after its objects are freed; worker exit provides an independent cache-lifetime
control. Do not run this probe or builds concurrently with a throughput campaign.

For a diagnostic-only cache sensitivity comparison, start a fresh control process:

```bash
env GLIBC_TUNABLES=glibc.malloc.tcache_count=0 target/debug/gewyvern_validate --json linux-tcp-capture --capture-memory-control --out-dir target/validation/capture-memory-control-no-tcache-new
```

The [glibc tunable](https://sourceware.org/glibc/manual/latest/html_node/Memory-Allocation-Tunables.html)
disables per-thread caching for that invocation only. Compare each run's own
first/last and min/max values, since argument lengths and thread bookkeeping
affect absolute Rust counts. To check the actual capture path, apply the same
per-process environment to a separate `--capture-soak` campaign. Never silently
apply it to production or mix its throughput with a default-cache benchmark.
Flat no-cache counters support cache retention in the paired experiment, not
an exact attribution of every C byte or an indefinite leak-freedom guarantee.

To vary packetization and connection identities, rather than equating each
64-byte echo with one kernel event, use the separate finite campaign:

```bash
sudo target/debug/gewyvern_validate --json linux-tcp-capture --capture-mixed --out-dir target/validation/capture-mixed-new
```

Four fresh-socket epochs exercise 1/7/8/63/64/65/511/4096-byte logical messages,
split server writes, four-message coalesced exchanges, and repeated variable
traffic. Every client half-closes after its final request and still verifies the
complete final response and EOF. Each epoch owns two selected clients plus one
unselected neighbor in the same cgroup and at the same listener endpoint.
The neighbor must complete its own full-byte echo without entering the capture.

The oracle checks contiguous per-cookie TCP payload sequence ranges, including
32-bit wraparound, and each actual captured prefix against its expected stream
offset. It reconciles selected application byte totals with captured payload
ranges and kernel/consumer/runtime event totals, **not message counts with
packet counts**. This follows TCP's byte-stream semantics in
[RFC 9293](https://www.rfc-editor.org/rfc/rfc9293.html#section-3.7).
Full-byte application checks do not make the 64-byte-prefix capture a full-payload
archive. Retransmission, overlap, gaps, foreign cookies, malformed framing or
wrong bytes fail closed; there is no generalized reassembly hidden in this oracle.

The 1-MiB ring, 256-fact runtime batches, two-core/512-MiB/32-task limits and
sampled same-adapter reanalysis remain. Each epoch has a 45-second bound and
checks owned link/pin/cgroup cleanup before the next fresh identity set. A final
HTTP boundary/replay scenario checks continued usability. This finite correctness
campaign is not a saturated throughput benchmark, protocol/vulnerability detector,
multi-host matrix or long-duration memory acceptance test.

The validator verifies ownership labels and removes resources by ID, including
on normal error unwinding. Check stderr after a cleanup failure; abrupt process
termination still requires manual cleanup of the recorded run-owned resources.
After setup both tracepoint and packet BPF link fds detach on process exit;
normal cleanup also restores the original
cgroup and removes the owned directory. A killed process can leave an empty
cgroup, pins or target resources requiring cleanup. The recovery supervisor only
reaps its explicitly recorded child resources, not arbitrary older runs or
container resources. Client response bodies remain
client I/O evidence; packet-local DSL candidates and origin-verified derived
prefixes are deliberately distinct.
This proves the client-to-container endpoint correlation and scoped HTTP prefix
classification, not container PID/server-socket
attribution, vulnerability detection, general live CLI capture or load acceptance.
Trust remains `unverified-local`; no release gate or tensor maturity is promoted.

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
cargo run --quiet --bin gewyvern_validate -- leserpent-benchmark
bash scripts/perf/benchmark_summary.sh
bash scripts/perf/trim_workspace_disk.sh --dry-run
bash scripts/perf/trim_workspace_disk.sh --apply
bash scripts/history/render_minor_line_ir_snapshot.sh http=dsl/http_request_path.gewy
```

Use `leserpent-benchmark` for the Leserpent 2 release workload. It
enforces broad cold-open, query, effect-throughput, UI document/patch/codec, and
release-binary-size budgets while retaining `benchmark-summary.json` and
`evidence-index.json`. Timing comparisons are valid only within the same
host class. The shell summary remains the Gewyvern 1.x ignored-benchmark helper.

Use `trim_workspace_disk.sh` when local iteration has left behind large
rebuildable artifacts. It only previews by default; `--apply` removes the
following while builds and applications are idle:

- Local Rust `dev` and `lean` profile outputs
- Ignored, untracked frontend `node_modules/`
- Ignored, untracked .NET `bin/` and `obj/`
- Ignored, untracked Python cache directories such as `__pycache__/`

The script intentionally skips source, docs, Git history, release bundles,
`target/package/`, `target/validation/`, installed applications, shared caches,
and `apps/leserpent/src/Leserpent/data`. It ignores `CARGO_TARGET_DIR` in favor
of the checkout's local target directory and refuses symlinked Cargo outputs.

## Suggested Reading Order

If you are new to the project and want to orient first, use:

1. [README.md](README.md)
2. [docs/index.md](docs/index.md)
3. [docs/script-entrypoints.md](docs/script-entrypoints.md)
4. [docs/field-validation.md](docs/field-validation.md)
5. [docs/release-checklist.md](docs/release-checklist.md)

That sequence gives you the product posture, the docs map, the script map, the
current validation posture, and the actual ship gate.
