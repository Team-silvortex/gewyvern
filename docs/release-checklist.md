# Release Checklist

This page is the shortest practical release checklist for the active
`1.4.0` line.

Use it when the question is not "how does packaging work?" or "what does field
validation mean?", but simply:

- can we still ship `1.4.0` with confidence?
- did we exercise the real packaged artifacts?
- did standalone and multi-project paths both survive?

For deeper background, see:

- [docs/field-validation.md](docs/field-validation.md)
- [docs/field-findings.md](docs/field-findings.md)
- [docs/packaging.md](docs/packaging.md)
- [docs/script-entrypoints.md](docs/script-entrypoints.md)
- [docs/history/v1.0.0.md](docs/history/v1.0.0.md)
- [docs/history/v0.20.x.md](docs/history/v0.20.x.md)
- [docs/history/v0.19.x.md](docs/history/v0.19.x.md)
- [docs/history/v0.18.x.md](docs/history/v0.18.x.md)

## Role In The Shelf

Treat this page as the shortest practical release gate.

Use it when the question is:

- can we call this line green today?
- which exact packaged checks must pass before shipping?
- what is the fastest narrowing path when one release-phase check fails?

Do not use this page as:

- the full validation philosophy for the line
- the durable statement of what the stable line is supposed to mean
- the evidence log of what already passed over time

For those, use:

- [docs/field-validation.md](docs/field-validation.md)
- [docs/history/v1.0.0.md](docs/history/v1.0.0.md)
- [docs/history/v0.20.x.md](docs/history/v0.20.x.md)
- [docs/field-findings.md](docs/field-findings.md)

## Companion Shelves

- [docs/field-validation.md](docs/field-validation.md)
  for the broader validation program and scenario bands
- [docs/field-findings.md](docs/field-findings.md)
  for the short record of what has already been demonstrated
- [docs/history/v0.19.x.md](docs/history/v0.19.x.md)
  for the current line's intended debugger-integration posture
- [docs/history/v0.18.x.md](docs/history/v0.18.x.md)
  for the protocol-breadth and physical-host validation baseline that this line
  inherits

## Current `1.4.0` Gate

Treat `1.4.0` as release-ready only when all of the following stay true:

1. current native artifacts are rebuilt from the current source tree
2. packaged `deb` and `rpm` install smoke both pass
3. packaged standalone runtime validation both pass
4. packaged protocol validation both pass
5. packaged operator-path validation both pass
6. runtime validation still proves the training dataset/export roundtrip
7. lifecycle validation proves startup, stop, log evidence, recovery, and cleanup
8. the default `deb+rpm` release wrapper passes as one routine
9. the three-module Docker stack smoke still passes
10. pathological container/runtime-ingest validation still proves bad clients do
    not wedge the runtime
11. debugger cross-validation still compares runtime summary JSON, debugger
    console JSON, and `gewyc` envelope JSON without overclaiming negative cases
12. security dependency checks stay clean for Rust, .NET, and frontend package
    manifests
13. control-plane and sidecar security boundary tests still pass for
    `leserpent` and `etragon`
14. the project status tensor validates, and every changed architecture,
    module, feature, dependency, blocker, and contract is reflected there

This section is intentionally binary and operational. It should stay shorter
and stricter than the broader validation note.

Validate project direction before rebuilding artifacts:

```bash
cargo run --bin gewyvern_status -- validate
cargo test --test project_status_tdd
```

## Rebuild Current Artifacts

Always rebuild the native packages before calling the release path green:

```bash
bash scripts/packaging/build_packages_in_container.sh --format all
```

Expected outputs:

- `target/packages/gewyvern_<version>-1_<deb-arch>.deb`
- `target/packages/rpm/gewyvern-<version>-1.<rpm-arch>.rpm`

The `<version>` value is read from the root `gewyvern` package metadata in
`Cargo.toml`. For the current tree, that resolves to `1.4.0`, so the concrete
artifact names should look like `gewyvern_1.4.0-1_<deb-arch>.deb` and
`gewyvern-1.4.0-1.<rpm-arch>.rpm`.

The package smoke must always verify the artifacts that the tree actually
builds today.

Do not trust an older green run if it was using stale artifacts from another
version line.

## Fastest Release Check

The shortest one-command gate is:

```bash
cargo run --quiet --bin gewyvern_validate -- release-gate
```

For CI or release bots that should consume one final machine-readable result
instead of scraping progress logs, use:

```bash
cargo run --quiet --bin gewyvern_validate -- --json release-gate
```

That sequence rebuilds current native artifacts, runs the packaged release
validation wrapper, runs the three-module stack smoke, and then runs the
pathological container/runtime-ingest validation.

If you want to skip one phase while narrowing a failure, use:

```bash
cargo run --quiet --bin gewyvern_validate -- release-gate --skip-build
cargo run --quiet --bin gewyvern_validate -- release-gate --skip-stack
cargo run --quiet --bin gewyvern_validate -- release-gate --skip-debugger-cross
cargo run --quiet --bin gewyvern_validate -- release-gate --skip-pathology
cargo run --quiet --bin gewyvern_validate -- release-gate --remote-host-validation
cargo run --quiet --bin gewyvern_validate -- release-gate --deb
cargo run --quiet --bin gewyvern_validate -- release-gate --rpm
```

The same narrowing paths also work with `--json` placed before the command:

```bash
cargo run --quiet --bin gewyvern_validate -- --json release-gate --skip-build
cargo run --quiet --bin gewyvern_validate -- --json release-gate --remote-host-validation
```

The lower-level packaged release-minded entrypoint is:

```bash
cargo run --quiet --bin gewyvern_validate -- release-container-check
```

This must pass in default `deb+rpm` mode.

It covers:

- package install smoke
- packaged runtime validation
- packaged protocol validation
- packaged operator-path validation

The packaged runtime validation now also confirms the machine-facing training
surface stays internally consistent:

- `/v1/latest/training-dataset.json` remains fetchable
- each sample row points to a usable `training-example.json`
- manifest `sample_id` values match the fetched sample payloads
- the default split policy remains `name_bucket_mod_10`

If you are narrowing a failure, these subchecks may be run independently:

```bash
cargo run --quiet --bin gewyvern_validate -- package-install-smoke
cargo run --quiet --bin gewyvern_validate -- container-runtime-validation
cargo run --quiet --bin gewyvern_validate -- container-protocol-validation
cargo run --quiet --bin gewyvern_validate -- container-operator-path-validation
cargo run --quiet --bin gewyvern_validate -- debugger-cross
cargo audit
```

If you need one real Linux host signal in addition to the local packaged gate,
run:

```bash
cargo run --quiet --bin gewyvern_validate -- remote-linux-host-validation
```

Or fold it into the main release gate:

```bash
cargo run --quiet --bin gewyvern_validate -- release-gate --remote-host-validation
```

When the remote host path is enabled through `release-gate`, the CLI now also
prints the resolved remote directory, the remote eBPF outcome, and the slowest
observed remote phases so you can narrow release friction without opening the
remote evidence directory first.
It also prints the recent remote eBPF trend and the newest recent-history
entries when local history is available.

In JSON mode, the final `release-gate` object now carries:

- top-level `schema_version = 1`
- `extra.stages.*` booleans for each major gate phase
- `extra.stages.debugger_cross_validation` for the local debugger readiness
  stage inside the main release gate
- `extra.gate_posture`, `extra.ship_signal`, and `extra.next_step` as the
  shortest overall ship/no-ship reading for the whole release gate
- `extra.remote = null` when the current run skipped remote validation
- `extra.remote.preflight`, `extra.remote.ebpf`, and
  `extra.remote.phase_timings` when the current run did execute the remote
  stage
- `extra.remote.total_seconds` as the full remote validation wall-clock total
  for quick regression comparison
- `extra.remote.budget_warnings` when a keyed remote phase exceeded the current
  soft release budget, including `remote_package_smoke` and
  `remote_runtime_smoke`
- top-level `extra.ship_signal = "timing_watch"` when remote Linux proof
  succeeded but the current host run exceeded one of the soft remote timing
  budgets
- `extra.remote.validation_posture`, `extra.remote.release_gate_signal`, and
  `extra.remote.next_step` for the quick ship/no-ship reading of the Linux host
  result
- `extra.remote.linux_proof_complete = true` may coexist with
  `extra.remote.release_gate_signal = "coverage_incomplete"`: the current host
  proved all attach paths, but the retained matrix still needs two physical
  hosts and two kernel releases
- `extra.remote.requires_followup = true` for partial attach proof, evidence
  integrity warnings, timing warnings, or incomplete physical-host coverage;
  any such remote signal propagates to the overall ship signal instead of
  being overwritten by the successful current-host smoke

Keep the practical Linux target-lab shelf as a separate artifact on purpose:

- `juice-shop-container-validation` is a high-signal optional Linux/BPF
  release-confidence check
- `ftp-denied-container-validation` is a high-signal optional Linux/BPF
  release-confidence check for rejected FTP authentication
- `ldap-bind-denied-container-validation` is a high-signal optional Linux/BPF
  release-confidence check for rejected LDAP binds
- it is not part of the default `release-gate.extra.stages.*` contract today
- callers that want it should run it explicitly and preserve its own evidence
  directory alongside the main release-gate JSON

After a successful `release-gate` run, also preserve:

- `target/validation/release-gate-artifacts.json`
- `target/validation/release-gate-artifacts.txt`

Those two companion files are the compact release-facing index of which
evidence shelves were present at the time, including whether the optional
`juice-shop-container` shelf existed yet.

Practical `jq` examples:

```bash
cargo run --quiet --bin gewyvern_validate -- --json release-gate \
  | jq '.ok and .extra.stages.release_container_check'

cargo run --quiet --bin gewyvern_validate -- --json release-gate --remote-host-validation \
  | jq '.extra.remote.ebpf.status'

cargo run --quiet --bin gewyvern_validate -- --json release-gate --remote-host-validation \
  | jq '.extra.remote.slowest_phase_entries'

cargo run --quiet --bin gewyvern_validate -- --json release-gate --remote-host-validation \
  | jq '.extra.remote.recent_ebpf_trend, .extra.remote.remote_ebpf_status_counts'
```

Interpret the remote Linux signal explicitly:

- `remote_ebpf_smoke` means a real Linux host had enough privilege to prove
  native attach/kprobe/tc smoke behavior.
- `remote_ebpf_smoke_skipped` means package/runtime confidence still passed,
  but Linux eBPF attach confidence was not established on that host because
  privilege or route-device prerequisites were missing. Treat that as an
  environment gap, not as a hidden green light.
- a same-day `remote_ebpf_smoke_skipped` run followed by an admin-assisted
  `remote_ebpf_smoke` run is acceptable, but preserve the full-ready evidence
  shelf for the final ship read.

For the dependency-vulnerability portion of the release gate, the current
practical commands are:

```bash
cargo audit
dotnet restore apps/leserpent/leserpent.slnx --locked-mode
dotnet list apps/leserpent/leserpent.slnx package --vulnerable --include-transitive
cd apps/leserpent && npm audit --json
cd apps/leserpent && npm audit --omit=dev --json
```

Treat this set as the current release-ready minimum for:

- Rust crates through `Cargo.lock`
- Leserpent's NuGet graph
- Leserpent's frontend package lock

## Expected Packaged Semantics

The release check is not only checking process exit codes. It is also asserting
current behavior that should remain stable enough for this line:

- `http request` stays `manual_review`
- `tls client` stays `manual_review`
- `quic initial` stays `collect_more_runtime_evidence`
- `http3 request` stays
  `operator_guidance_action = "safe_to_escalate_protocol_signal"`
- packaged malformed ingest does not kill the `--serve` loop
- packaged training dataset roundtrip still verifies stable sample identity

If one of these changes, treat it as a deliberate semantics review, not just a
test refresh chore.

## Multi-Project Integration Gate

After the single-project packaged path is green, run:

```bash
bash scripts/validation/three_module_stack_smoke.sh
```

On physical validation hosts with an already-built stack image, this equivalent
form avoids rebuilding the Docker toolchain while still refreshing leserpent's
NuGet packages before using `--no-restore`:

```bash
IMAGE_TAG=gewyvern-stack-dev-physical \
  SKIP_DOCKER_BUILD=true \
  LESERPENT_DOTNET_RESTORE_FIRST=true \
  LESERPENT_DOTNET_IGNORE_FAILED_SOURCES=true \
  LESERPENT_DOTNET_NO_RESTORE=true \
  bash scripts/validation/three_module_stack_smoke.sh
```

That smoke should confirm:

- `etragon-status-ok`
- `etragon-output-ok`
- `summary-ok`
- `runtimes-ok`
- `three-module stack smoke: ok`
- one `resilience_summary=...` artifact path worth archiving with the current
  line's review notes when collaboration posture is relevant

This is the highest-signal collaboration check for the current line because it
exercises:

- two `gewyvern` runtimes
- one nearby `etragon` sidecar
- one `leserpent` control plane

## Pathological Container Runtime Gate

After normal stack confidence is green, run:

```bash
bash scripts/validation/pathological_container_validation.sh
```

That gate drives intentionally bad clients against the runtime ingest surface:

- truncated JSON
- empty disconnects
- slow-drip incomplete JSON
- oversized fact lines

The expected result is not "nothing bad happened"; it is more precise:

- the runtime stays reachable after bad input
- health and resilience surfaces report degraded/backing-off state
- post-fault analysis still returns a coherent runtime payload
- log evidence records the ingest failures without turning them into process
  death

## If Something Fails

Use this triage order:

1. if package install smoke fails, inspect packaging/layout first
2. if runtime validation fails, inspect `--serve`, socket ingest, API lifecycle, or packaged assets
3. if protocol validation fails, compare current JSON semantics against the scripted expectation
4. if operator-path validation fails, check whether the runtime drifted or the expected guidance drifted
5. if three-module smoke fails, inspect cross-project API contracts before changing single-project diagnosis logic
6. if pathological validation fails, inspect socket ingest resilience and bad-client log evidence before changing protocol diagnosis logic

## Ship Read

For the active `1.4.0` line, a good practical ship read is:

- current artifacts rebuilt
- `release_gate.sh` green, or the equivalent build + packaged release check +
  three-module smoke sequence green
- full `release_container_check.sh` green in default mode
- `three_module_stack_smoke.sh` green
- `pathological_container_validation.sh` green on a Docker-capable host
- optional but high-signal: `juice-shop-container-validation` green on a Linux
  host with BPF attach privileges
- `gewyvern_validate -- debugger-cross` green
- Rust/.NET/frontend dependency vulnerability checks green
- no new drift in `field-findings` that would downgrade trust in conservative diagnosis

If all of these are true, the line is in a healthy release posture.
## Security Shelf

Run the lightweight security shelf before calling the line green:

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

The Leserpent security project explicitly declares `IsTestProject=true` for
.NET 10. A successful command must report a nonzero discovered and executed
test count; a build-only exit with no test summary is not release evidence.

What this shelf proves:

- dependency advisories are still clean across Rust, NuGet, and frontend
  packages
- `leserpent` still rejects remote control-plane access without the right token
  and still enforces the loopback intent header rules in the real middleware
  chain
- `etragon` still rejects duplicate `Content-Length` ambiguity and token-header
  ambiguity instead of silently accepting a risky request shape
