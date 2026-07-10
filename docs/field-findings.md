# Field Findings

This note records the highest-signal findings from the active `0.20.x` line
while preserving a shorter historical tail from the earlier `0.15.x`
field-validation phase.

It is intentionally short.

It is not a replacement for:

- [docs/field-validation.md](docs/field-validation.md)
- [docs/history/v0.20.x.md](docs/history/v0.20.x.md)
- [docs/history/v0.19.x.md](docs/history/v0.19.x.md)
- [docs/history/v0.18.x.md](docs/history/v0.18.x.md)

Instead, it answers a narrower question:

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
- the main statement of what the line should mean

For those, use:

- [docs/field-validation.md](docs/field-validation.md)
- [docs/release-checklist.md](docs/release-checklist.md)
- [docs/history/v0.20.x.md](docs/history/v0.20.x.md)

## Current `2026-07-10` Findings From `0.20.x`

### 1. The Default Release Gate Now Passes As One Routine

The current release-style entrypoint now passes in one run:

- `cargo run --quiet --bin gewyvern_validate -- release-gate --skip-build`

That green path includes:

- packaged release validation in default `deb+rpm` mode
- the three-module stack smoke
- pathological container validation

This is stronger than a collection of isolated green commands because it proves
the current orchestration order also holds up.

### 2. Packaged Linux Validation Is Green Across Both Package Families

The current packaged release routine is green in default mode through:

- package install smoke
- packaged runtime validation
- packaged protocol validation
- packaged operator-path validation

That means the active `0.20.0` native artifacts are usable not only as local
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

Fresh `0.15.0` native packages were rebuilt from the current source tree and
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

## Practical Read Of The Current Line

The current `0.20.x` line now looks strong in these ways:

- protocol/package shelf is stable
- current `0.20.0` native artifacts are the ones being exercised
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
