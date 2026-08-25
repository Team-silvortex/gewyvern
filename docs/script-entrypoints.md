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
cargo dev build
cargo dev package linux --format layout
cargo dev package desktop
cargo dev deploy desktop --launch
```

`cargo dev version check` verifies the shared Cargo/.NET/lockfile/documentation
release identity. `cargo dev version set VERSION [--dry-run]` updates those
current-version surfaces transactionally while leaving historical evidence and
Git tags untouched. `cargo dev build` runs the Rust, Leserpent control, and
Avalonia builds in parallel with locked dependency behavior. The package and
deploy commands keep atomic output boundaries and report their final artifact.
Use the scripts and specialized Rust binaries below when debugging an
individual stage. Formal macOS release packaging uses the same desktop command
with the paired `--identity` and `--notary-profile` options.

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

To fold a Leserpent Apple readiness report into the same machine gate without
running signing commands again:

```bash
cargo run --quiet --bin gewyvern_validate -- --json release-gate \
  --macos-release-preflight docs/fixtures/leserpent_macos_release_preflight.json
```

The input is bounded and strictly cross-checked. A consistent blocked report is
retained as evidence and marks `extra.stages.macos_release_preflight_blocked`,
while malformed or contradictory evidence fails the command. When all product
and Linux stages pass but Apple credentials remain absent, the aggregate signal
is `apple_credentials_blocked`, never `ready`.

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
privileged commands compile smoke sources only from the validator's build-time
workspace root; changing the remote working directory cannot substitute loader
or BPF source files.
This is a compatibility path. New hosts should install the fixed helper and
must not grant the validation account unrestricted passwordless `sudo`, or
authorize `env`, a shell, or workspace binaries in `sudoers`.
Packaged hosts can configure the fixed path with
`sudo gewyvern-ebpf-provision --user VALIDATION_USER`; add `--dry-run` to
validate the account and installed helper without writing configuration. This
root-only management binary is deliberately absent from the generated sudoers
allowlist.
One run can cover package/runtime smoke, the Leserpent
control-plane NativeAOT persistence proof, and attach/kprobe/tc in one evidence
transaction. A first run under a new workspace identity may report `watch`
because its isolated source and target caches are cold; rerun with the same
ordinary identity to measure the warm reference. A clean warm run reports
`linux_proof_complete=true`.
It still reports `coverage_incomplete` until successful evidence spans at least
two physical host fingerprints and two kernel releases.

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

Useful flags:

- `--host <ssh-host>`
- `--target-kind <physical|vm>`
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
- `target/validation/remote-linux-host-validation/leserpent-control-plane-aot-linux-x64/`
- `target/validation/remote-linux-host-validation/leserpent-language-pack-local-orchestra-native-aot-linux-x64/`
- `target/validation/remote-linux-host-validation/remote-phase-timings.txt`
- `target/validation/remote-linux-host-validation/remote-run.txt`
- `target/validation/remote-linux-host-validation/remote-ebpf-history.jsonl`
- `target/validation/remote-linux-host-validation/remote-ebpf-history-rejected.jsonl`
- `target/validation/remote-linux-host-validation/remote-ebpf-latest.json`
- `target/validation/remote-linux-host-validation/remote-ebpf-recent.txt`
- `target/validation/remote-linux-host-validation/remote-ebpf-status-summary.json`

With `--target-kind vm`, the same bounded inventory is written under
`target/validation/remote-linux-vm-validation/` instead. The two complete run
locks, histories, latest records, and matrix summaries never share a path.
The local workspace-key optimization is not evidence and is isolated separately
under `target/validation/remote-workspace-sync-cache/{physical,vm}.txt`.

The phase-timing file records the observed wall-clock time for each major
remote validation step so we can tell whether regressions come from sync,
materialization, build, the Leserpent control-plane NativeAOT proof, package
smoke, the Avalonia Local Orchestra plus saved-daemon language-pack NativeAOT proof, runtime
smoke, or the privileged eBPF attach path.
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
`matrix.ready` is always false for VM evidence.
An `ok` attach run still reports `validation_posture=full`, but the release
signal is `coverage_incomplete` and `requires_followup=true` while this matrix
is below threshold. `release_gate_signal=ready` is reserved for a successful
current run whose retained physical-host matrix is also ready.
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
fixtures on the supported native host. It verifies unique Automation IDs,
complete names, explicit action labels, HelpText mapping, and the 4.5 WCAG AA
text-contrast floor. Linux uses `xvfb-run`; the validator never installs it or
invokes `sudo`. The summary and per-fixture logs are retained under
`target/validation/leserpent-accessibility/`.

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
strips group/world write permissions and atomically replaces managed links. It
rejects an existing unmanaged launcher
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
produce `release_ready=true`. The current retained fixture at
`docs/fixtures/leserpent_macos_release_preflight.json` is explicitly blocked by
the absent identity and unrequested profile; it is readiness evidence, not an
Apple release claim.

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
10 core capability families, nine permitted closure-work categories, six
explicit deferrals, both authority-document anchors, and their live status-cell
references. Scope expansion, a missing deferral, an Etragon/1.x authority leak,
or a stale status reference fails closed. This manifest contains no executable
arguments and reports `scope_freeze_ready=true` independently of the still
candidate version-1 schema inventory.

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
cargo run --quiet --bin gewyvern_validate -- leserpent-benchmark
bash scripts/perf/benchmark_summary.sh
bash scripts/perf/trim_workspace_disk.sh --dry-run
bash scripts/perf/trim_workspace_disk.sh
bash scripts/history/render_minor_line_ir_snapshot.sh v0.15.x
```

Use `leserpent-benchmark` for the Leserpent 2 release workload. It
enforces broad cold-open, query, effect-throughput, UI document/patch/codec, and
release-binary-size budgets while retaining `benchmark-summary.json` and
`evidence-index.json`. Timing comparisons are valid only within the same
host class. The shell summary remains the Gewyvern 1.x ignored-benchmark helper.

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
