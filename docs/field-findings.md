# Field Findings

This note records the highest-signal findings inherited by the active `2.5.x` line
while preserving a shorter historical tail from the earlier `0.15.x`
field-validation phase.

It is intentionally short.

It is not a replacement for:

- [docs/field-validation.md](docs/field-validation.md)
- [docs/history/v1.0.0.md](docs/history/v1.0.0.md)
- [docs/history/v0.20.x.md](docs/history/v0.20.x.md)
- [docs/history/v0.19.x.md](docs/history/v0.19.x.md)
- [docs/history/v0.18.x.md](docs/history/v0.18.x.md)

Instead, it answers a narrower question for the stable line:

- what has already been observed in real validation work
- what currently looks stable
- what still looks conservative rather than strongly diagnostic

## Role In The Shelf

Treat this page as the short evidence ledger.

Use it when the question is:

- what has already been demonstrated recently?
- which release-line claims are backed by concrete observed runs?
- where did we already discover drift or conservative gaps?

Do not use this page as:

- the validation program definition
- the shortest release gate
- the main statement of what the stable line should mean

For those, use:

- [docs/field-validation.md](docs/field-validation.md)
- [docs/release-checklist.md](docs/release-checklist.md)
- [docs/history/v1.0.0.md](docs/history/v1.0.0.md)
- [docs/history/v0.20.x.md](docs/history/v0.20.x.md)

## `2026-11-04` Kernel Payload Prefix To Gewylang Classification

The container capture path now passed three physical-host runs with an actual
`cgroup_skb/ingress` BPF sampler: ten TCP state events and two nonempty packet
snapshots per run, two reconstructed flows, and zero reported capture loss.
Validation-only Gewylang rules identified HTTP/1.1 403 and 500 from the captured
bytes. Their socket cookies and prefixes matched independently saved client
responses; the unselected HTTP 200 request remained absent. Facts, flows,
reasons, program flows and protocol IR replayed identically. The original
loopback mode also passed. See the [real prefix sample and proof hashes](fixtures/gewyvern_packet_capture_linux_x86_64_20261104.json).

The first attempt exposed a verifier-visible zero-length read bound after LLVM
folding. The sampler now explicitly proves a 1..64-byte read before calling the
helper. That error path removed its owned resources too. Packet and TCP counter
files are separate so one capture cannot overwrite the other's evidence.
Final-source Linux regressions passed 721 unique tests, including two explicitly
enabled socket tests; seven performance benchmarks were not run. Strict Clippy
including test targets and format checks passed. No macOS build or image pull
was used, and no release gate or tensor maturity was promoted.

This remains a bounded validation backend: only two owned private-endpoint
client sockets, ingress IPv4/TCP payloads, 128 events and 64 bytes per prefix.
Every BPF path allows traffic without modification. A temporary cgroup is
removed after restoring the validator's original placement, and the BPF link
is fd-owned. No unrelated process is moved. The sampler records real TCP header
fields; network namespace comes from the correlated kernel TCP state, not a
fabricated PID. Kernel skb snapshots do not imply on-wire packet boundaries.
There is no TCP stream reassembly, TLS decryption, vulnerability verdict,
server-socket attribution or general live-CLI claim. Trust is `unverified-local`.

## `2026-11-04` Container Request To TCP Flow Correlation

The fixed `linux-tcp-capture --juice-shop` mode passed three independent runs on
the same physical Ubuntu `7.0.0-42-generic` host. Each preserved file-guard HTTP
403 and SQL-error HTTP 500 responses, matched ten kernel TCP state events to two
client sockets and reconstructed flows, and excluded a third HTTP 200 request.
Every run reported zero dropped events and replayed identical facts, flows and
reasons. The [real sample and hashes](fixtures/gewyvern_container_capture_linux_x86_64_20261104.json)
retain the cookie/tuple/netns/time/fact-ID joins. The original loopback mode also
passed again after sharing the explicit address-scoped capture backend.
Relevant Linux regressions passed 707 unique tests, including explicitly enabled
TCP/Unix input tests, and targeted strict Clippy passed. Seven performance
benchmarks were not run. The code and raw sample hashes were cross-checked;
only small reports/HTTP evidence were copied to
`target/validation/container-capture-20261104/` on macOS, with no local build.

The first attempt stopped when this Docker internal network did not publish the
requested host port. Isolation was retained: the final path publishes no ports
and connects only to the inspected target IP from its owned bridge gateway.
The second attempt caught chunked HTTP error responses; bounded, strict chunk
framing is now supported, and raw client responses are saved before parsing.
Failed-run logs are retained. The dedicated container, network and BPF pins
were removed on success and both error paths; unrelated services were not used.

The client observations are kept in a separate correlation report. No HTTP
packet facts or PID lineage are synthesized into the kernel export. This is a
bounded client-to-container evidence chain, not BPF payload classification or
vulnerability detection. Sustained load, server-side socket attribution and
multi-host/kernel acceptance remain unproven; trust stays `unverified-local`.

## `2026-11-04` Real TCP Capture And Replay

The same physical Ubuntu x86_64 host (`7.0.0-42-generic`) passed three independent
`linux-tcp-capture` runs: each recorded seven real kernel TCP state events,
reconstructed two owned flows, reported zero dropped events and excluded the
unselected decoy socket. Established, half-closed and closed snapshots replayed
with identical facts, flows and reasons. See the
[captured sample and proof hashes](fixtures/gewyvern_tcp_capture_linux_x86_64_20261104.json).

This uncovered and fixed two product bugs:

- Flow reconstruction treated TCP state numbers as ordered liveness. In
  particular `CLOSE_WAIT` was prematurely terminated. Terminal states are now
  explicit; reconnecting on the same socket clears stale termination evidence.
- TCP fact export omitted both IP addresses and replay silently replaced them
  with zero. New exports preserve all 16 address bytes. Legacy files without
  addresses remain readable; present but malformed address arrays are rejected.

The initial live runs also exposed the host bpftool's nested BTF output format;
the adapter now handles it without guessing untyped raw bytes. Failed-run logs
are retained, and run-owned BPF pins were cleaned on failure as well as success.
Compilation and tests stayed on Linux; no macOS build or image pull was needed.
The final relevant regression set passed 679 unique tests, including explicitly
enabled TCP/Unix input tests, plus targeted strict Clippy. Seven performance
benchmarks were not run and are not counted as passes. Only small reports and
capture JSON were copied to `target/validation/tcp-capture-20261104/` on macOS.

This proves the bounded owned-socket kernel-event-to-runtime path. It does not
prove HTTP payload analysis, vulnerability detection, PID lineage, general live
CLI capture or container-target request correlation. The HTTP 204 was observed
by the test client, not inferred by Gewyvern. Trust stays `unverified-local` and
no status-tensor maturity or release gate was promoted.

## `2026-11-04` Post-Split Linux Field Lab

The current working tree passed its first post-split core validation campaign on
one physical Ubuntu x86_64 host running `7.0.0-42-generic`. Compilation, regression
tests and process-level scenarios ran in a resource-bounded Rust 1.98.0 container
on that host; actual BPF attachment ran on the host itself. No macOS compilation
or image pull was used. See the [bounded proof record](fixtures/gewyvern_field_lab_linux_x86_64_20261104.json).

- All 361 protocol packages passed registry validation; high-frequency/mixed-flow,
  debugger-cross, runtime-operator, runtime-lifecycle and socket/full-scan field
  smoke passed. The latter runtime suites include malformed-input recovery.
- 138 relevant unit/integration tests and targeted strict Clippy passed on Linux.
- Cached Juice Shop, FTP and LDAP targets preserved HTTP 403/SQL-error 500, FTP
  530/server denial, and LDAP 49/server bind-denial evidence respectively. Each
  target also passed tracepoint, kprobe and dedicated-interface TC attach checks.
- The lab entrypoints now require an explicit existing `GEWY_CONTAINER_TC_DEVICE`
  before Docker effects. A run-owned dummy interface replaced automatic primary
  route selection; missing targets were rejected by all three entrypoints.
- Target containers and the dummy interface were removed. Existing non-lab
  containers, the default route and non-transient qdisc state were unchanged.

Initial isolated-container failures came from missing cached test dependencies
and a non-writable default HOME. Fetching the locked dependencies and supplying
a dedicated writable HOME resolved them. The cached Rust image lacked Clippy;
it was installed in a temporary checking container, not on macOS or the host.
The host's default sudo-rs rejected a legacy sudoers wildcard rule; this run used
the existing password-authenticated `sudo.ws`, without changing system policy.

This proves target-side evidence preservation plus same-host attach capability,
not direct vulnerability classification or correlation of the target requests
into Gewyvern's eBPF-derived session output. `resilience-roundtrip` generated
helpers only and is not counted as live fault injection. Sustained-load/loss/tail
latency measurements, full external-engine fault injection and a second host or
kernel remain open. No status-tensor maturity or release gate was promoted.

Full evidence and build caches remain in the remote dedicated Gewyvern workspace;
only small top-level reports were copied to `target/validation/field-lab-20261104/`.
The pinned target image digests in the proof make the cached lab reproducible
without relying on a future `latest` tag.

## `2026-07-18` Linux Publication-Hardening Finding

A physical Ubuntu `6.17.0-35-generic` run exposed a Linux-only ownership drift
in the eBPF loader that macOS compilation could not see. After changing the
probe API to borrow dynamic plan strings and own only emitted failure evidence,
the complete remote validation passed: Linux x86_64 release build, atomic
DEB/RPM publication, strict manifest verification, package/runtime smoke, and
tracepoint, kprobe, and tc eBPF attach. The retained failed workspace was
removed after the successful rerun; synchronized evidence remains under
`target/validation/remote-linux-host-validation`.

The follow-up added a remote Linux all-target compile phase. Its first run
showed that an unanchored rsync `tests/` exclusion had removed Etragon's nested
inline-test module; anchoring the exclusion to the repository root preserved
nested crate/application tests without syncing the root integration shelf. The
corrected physical-host run passed the compile phase and the complete release
path, and the temporary failed workspace was removed.

Because the root integration shelf is only about 1.6 MiB and contains the
Linux-only eBPF tests, it is now synchronized as well. The first complete
compile found three stale borrowed-string assignments in `linux_smoke_tdd`;
after converting them to owned IDs, the full root and nested test targets plus
the release/package/runtime/eBPF path passed on the physical host.

Remote preflight evidence now also pins bounded first-line toolchain identity.
The verified host reported Rust/Cargo `1.95.0`, dpkg-deb `1.22.6`, and
RPM/rpmbuild `4.18.2`; the same values are retained in the latest eBPF history
entry so later failures can be compared against environment drift.

Strict shared parsing of remote preflight, artifact, and eBPF key/value evidence
then exposed that human attach summaries had been mixed into the eBPF stdout
protocol. Redirecting those summaries to stderr preserved operator visibility
while making stdout machine-only. The physical-host rerun passed with a clean
five-entry history and five successful eBPF outcomes.

Remote build, package-smoke, and runtime-smoke timings are now mandatory strict
evidence rather than best-effort files. The physical run produced complete,
canonically ordered finite timing sets for all core phases (with unpack refresh
remaining the only optional cache phase) and completed with six consecutive
successful eBPF outcomes.

## `2026-07-10` Findings Inherited From `1.0.0`

### 1. The Default Release Gate Now Passes As One Routine

The current release-style entrypoint now passes in one run:

- `cargo run --quiet --bin gewyvern_validate -- release-gate --skip-build`

That green path includes:

- packaged release validation in default `deb+rpm` mode
- the three-module stack smoke
- pathological container validation

This is stronger than a collection of isolated green commands because it proves
the current orchestration order also holds up.

The current release-facing artifact trail is also easier to preserve now
because each successful `release-gate` run refreshes:

- `target/validation/release-gate-artifacts.json`
- `target/validation/release-gate-artifacts.txt`

Those two files are the compact index of which release shelves were present at
the time, including optional high-signal evidence such as
`target/validation/juice-shop-container`.

### 2. Packaged Linux Validation Is Green Across Both Package Families

The current packaged release routine is green in default mode through:

- package install smoke
- packaged runtime validation
- packaged protocol validation
- packaged operator-path validation

That means the active tree's native artifacts are usable not only as local
build outputs, but as real installed runtime inputs across both `deb` and
`rpm` flows.

### 3. The Multi-Project Stack Now Holds Up Under Resilience Checks

The current Docker collaboration smoke now passes through:

- two `gewyvern` runtimes
- one nearby `etragon` sidecar
- one `leserpent` control plane

The observed green outputs include:

- `etragon-status-ok`
- `etragon-output-ok`
- `summary-ok`
- `runtimes-ok`
- `gw-a-resilience-ok`
- `gw-b-resilience-ok`
- `gw-b-health-degraded-ok`
- `gw-b-resilience-degraded-ok`

This is currently one of the clearest signals that the monorepo collaboration
story is not only documented but operational.

### 4. Pathological Ingest Currently Degrades And Recovers Correctly

The current pathological container gate now passes with all intended bad-input
classes:

- truncated JSON
- empty disconnects
- slow-drip incomplete JSON
- oversized fact lines

The important current finding is not merely that the runtime "survived". It is
that degraded health, degraded resilience, post-fault analysis, and log
evidence all remain observable after the bad clients run.

### 5. Dependency Vulnerability Checks Are Currently Clean

The current release-ready security checks are green for:

- Rust via `cargo audit`
- Leserpent NuGet packages via
  `dotnet list apps/leserpent/src/Leserpent/Leserpent.csproj package --vulnerable`
- Leserpent frontend dependencies via `npm audit`

That closes one of the last release-checklist items that should not remain
implicit before a `1.0` push.

### 6. A Practical Linux Target-Lab Shelf Now Exists

The current Linux host validation work now also has one practical target-side
lab shelf through:

- `cargo run --quiet --bin gewyvern_validate -- juice-shop-container-validation`

The observed green run on `kyuubiki-lab` preserved:

- a file-guard style `403` response carrying
  `Only .md and .pdf files are allowed!`
- a SQL-style `500` response carrying `SQLITE_ERROR: incomplete input`
- same-host `linux-attach-smoke`
- same-host `linux-kprobe-smoke`
- same-host `linux-tc-smoke`

This matters because it is stronger than a synthetic protocol demo while still
being honest about the current product shape:

- `gewyvern` preserved suspicious target evidence
- Linux attach proof still held on the same host
- the result is not yet a claim of direct web-vulnerability classification

## Historical Stable Findings From `0.15.x`

### 1. Registry Validation Is Fully Green

At that point, the built-in protocol registry contained `90` package entries,
and the entire scanned registry passed:

- machine-facing `gewyc envelope --json` validation
- `gewyvern --scan-all --json --summary-only`

This means the current stable protocol shelf is no longer drifting at the
compiler/package level.

### 2. `0.15.0` Native Artifacts Drove The Validation Path

The packaged container path is no longer relying on stale historical artifacts.

Fresh `0.15.0` native packages were rebuilt from the then-current source tree and
then used as the input for the packaged validation chain:

- `target/packages/gewyvern_0.15.0-1_arm64.deb`
- `target/packages/rpm/gewyvern-0.15.0-1.aarch64.rpm`

This matters because an earlier validation pass could still succeed while
quietly exercising an older `0.10.0` package set.

That ambiguity was removed for the `0.15.x` line and remains useful context
for later release validation.

### 3. Packaged Linux Protocol Support Works After Real Install

Both native package families now pass packaged protocol validation inside clean
containers:

- Debian-family install via `.deb`
- RPM-family install via `.rpm`

The packaged validation path now covers:

- `DNS`
- `HTTP`
- `TLS`
- `HTTP/3`
- `QUIC`
- `SSH`
- `SOCKS5`
- `MySQL`
- `PostgreSQL`
- `SMTP`
- `LDAP`
- packaged `--scan-all`

This matters because it confirms that installed asset lookup, protocol registry
discovery, and packaged CLI behavior are working outside the development tree.

### 4. Packaged Standalone Runtime Works In Clean Linux Containers

Installed `.deb` and `.rpm` packages now pass real packaged runtime validation:

- packaged `--serve`
- packaged socket ingest
- packaged `/health`
- packaged `/v1/latest/summary.json`
- packaged `/v1/latest/analysis.json`
- packaged `/v1/latest/export.json`

Malformed ingest was also exercised without killing the packaged service loop.

That gives us a stronger signal than unit tests alone that the standalone
runtime shape survives real install workflows.

### 5. High-Value Operator Paths Already Look Conservative And Coherent

Packaged operator-path validation in clean Linux containers now covers:

- `DNS -> QUIC -> HTTP/3`
- `DNS -> TLS -> HTTPS CONNECT`
- `DNS -> SOCKS5 -> HTTPS CONNECT`
- `DNS -> TLS -> Postgres`
- `DNS -> SMTP`

The current important observation is not that all of these paths land in strong
final diagnosis.

The important observation is that they land in coherent, non-wildly-drifting
states:

- DNS remains a conservative advisory path
- TLS and HTTPS CONNECT remain healthy-but-advisory paths
- QUIC, SOCKS5 auth, PostgreSQL query, and SMTP session remain
  `missing_transition` paths with `collect_more_runtime_evidence`

That is a good prelaunch shape for a standalone debugger: the runtime is
preferring stable conservatism over premature collapse.

### 6. The Release Validation Path Now Holds Up Under Default `deb+rpm` Mode

The release-style compatibility layer exposed one real scripting bug during
this validation cycle:

- `cargo run --quiet --bin gewyvern_validate -- release-container-check`
- `cargo run --quiet --bin gewyvern_validate -- container-validation-summary`

In default `deb+rpm` mode, the underlying wrapper path could trip `set -u` because it
expanded an empty `mode_args` array directly.

That is now fixed, so the native release path can be used as a real release
entrypoint instead of only the explicit `--deb` / `--rpm` submodes.

### 7. A Release-Style Packaged Linux Checklist Already Passes

The current release-style container validation path now passes as one
deliberate checklist run across both package families:

- package install smoke
- packaged runtime validation
- packaged protocol validation
- packaged operator-path validation

This matters because it is stronger than saying “individual scripts look good”.

It means the packaged Linux path can already be exercised as one release-minded
routine, not only as disconnected local checks.

### 8. HTTP/3 Validation Expectations Are Now Aligned With Current Guidance

This validation cycle also exposed one important expectation drift:

- the current `http3 request` packaged path now lands in
  `operator_guidance_action = "safe_to_escalate_protocol_signal"`
- the older container assertions still expected
  `operator_guidance_action = "manual_review"`

The implementation and the focused HTTP/3 tests already agreed with the newer
behavior. What was stale was the packaged validation expectation.

That mismatch is now corrected, so the container suite reflects the current
diagnosis semantics instead of an older advisory-only assumption.

### 9. The Three-Module Stack Already Works In Containers

The current collaboration topology:

- `leserpent -> many gewyvern`
- `etragon <-> one nearby gewyvern`

now passes a real Docker smoke through:

- two `gewyvern` runtimes
- one nearby `etragon` sidecar
- one `leserpent` control plane

The stack smoke currently verifies:

- both `gewyvern` runtimes publish latest snapshot state
- `etragon` publishes additive output with augmentations
- `leserpent` can register both runtimes
- fleet summary reflects the paired sidecar correctly
- runtime detail reflects both plain and sidecar-backed nodes correctly

This is one of the highest-signal findings in the current line, because it
shows that the collaboration model is not only documented; it already works in
an isolated container environment close to the intended topology.

## Current Conservative Findings

### 1. Some “Denied” Demo Entries Do Not Yet Produce Strong Denial Semantics

Current packaged and local synthetic demo validation shows that entries such as:

- `socks5 auth-denied`
- `socks5 auth-connect-denied`
- `smtp rcpt-denied`
- `smtp data-denied`

still resolve to:

- `primary_failure_basis = "missing_transition"`
- a DNS/setup-oriented failure posture
- `operator_guidance_action = "collect_more_runtime_evidence"`

instead of a stronger denial-style diagnosis.

For the current line, this is treated as a conservative result rather than a
bug, because it avoids over-claiming on synthetic evidence that does not yet
drive the path far enough.

It does mean that these demo entries should currently be read as:

- “do not overtrust strong denial semantics here yet”

rather than:

- “this path is already a rich negative diagnosis oracle”

### 2. Packaged Negative-Path Validation Currently Focuses On Non-Overcollapse

The strongest current negative-path guarantee in packaged validation is:

- denial-style demo entries do not over-collapse into stronger claims when the
  evidence is still only setup-incomplete

That is useful and intentional, but it is not the same thing as saying:

- every negative demo path already yields a strong, human-like final diagnosis

## Strict Remote Evidence Consumption

The `2.5.x` validation CLI now consumes synchronized Linux-host evidence with
the same fail-closed posture used by the producer. Key-value evidence has a
shared bounded unique-key codec, phase timings require complete finite schemas,
and local history JSON and recent-history text have explicit file and record
limits. Human and JSON summaries reuse one parsed value, so they cannot disagree
because of a second read.

The latest physical Linux package, runtime, and eBPF run completed successfully
before this consumer-only hardening and extended the retained eBPF trend to
`7/7`. The hardened consumer is additionally covered by the real summary
fixture and malformed, duplicate, unknown, oversized, overlong, and
control-character negative tests.

## Non-Vacuous Leserpent Security Proof

The Leserpent parity/recovery shelf now executes the .NET control-plane security
project as a first-class proof suite instead of relying on a manual release
command. Its parser requires one internally consistent nonzero `Passed!`
summary and locked, proof-local build artifacts. Current macOS arm64 and
physical Linux x86_64 runs both completed 13 suites with 231 observed tests and
155 invariants; the xUnit contribution was 72 passed, zero failed, and zero
skipped. The Linux summary binds this result to kernel `6.17.0-35-generic`,
Rust/Cargo `1.95.0`, and .NET `10.0.109`. Proof-local .NET artifacts were
removed after evidence capture.

The shelf now removes its known summary, index, and suite logs before starting.
A failed rerun therefore cannot leave an earlier green summary available as if
it described the attempted run.

## Practical Read Of The Current Line

The current `2.5.x` line inherits strength in these areas:

- protocol/package shelf is stable
- current tree-native artifacts are the ones being exercised
- packaged standalone runtime works
- packaged high-frequency protocol families work
- packaged operator paths stay conservative and coherent
- packaged Linux release-style validation can run as one checklist in default
  `deb+rpm` mode
- the current three-module collaboration topology already works in Docker
- pathological ingest validation currently preserves degraded-but-live runtime
  behavior
- dependency vulnerability checks are currently green across Rust, .NET, and
  frontend lockfiles

The current line should still be read cautiously in these ways:

- some synthetic “denied” entries are still setup-shaped rather than richly
  denial-shaped
- the runtime is currently more trustworthy as a conservative diagnosis engine
  than as an aggressively collapsing one

That is still a good prelaunch posture.

It is much safer for first release validation than a system that appears more
confident than its evidence really supports.

## Visible Postlaunch Follow-Ups

These items are already visible, but they should stay out of the prelaunch
line unless field validation shows drift rather than conservatism.

### 1. Richer Negative-Path Semantics For Denied Demo Entries

Observed today:

- `socks5 auth-denied`
- `socks5 auth-connect-denied`
- `smtp rcpt-denied`
- `smtp data-denied`

currently stay in a setup-shaped conservative posture:

- `primary_failure_basis = "missing_transition"`
- DNS/setup-oriented stage selection
- `operator_guidance_action = "collect_more_runtime_evidence"`

This is not a prelaunch blocker because it avoids false strong conclusions.
It is still worth revisiting after launch so these entries can eventually carry
stronger denial semantics when the synthetic path genuinely drives far enough.

### 2. Stronger Failure-Oriented Packaged Operator Paths

Observed today:

- packaged operator-path validation is strong on conservative mixed/setup paths
- it is still thinner on richly denial-shaped packaged paths

This is not a prelaunch blocker because the more important guarantee is that
packaged installs behave coherently and do not invent overconfident conclusions.

### 3. Better Separation Between Advisory Paths And Stronger Final Diagnoses

Observed today:

- the runtime is more trustworthy as a conservative diagnosis engine than as an
  aggressively collapsing one
- this is good for launch trust, but it leaves some demo paths more advisory
  than an operator may hope

That is still the right prelaunch posture. After launch, high-value protocol
families can be revisited one by one to decide where stronger final diagnoses
are actually justified by evidence.
