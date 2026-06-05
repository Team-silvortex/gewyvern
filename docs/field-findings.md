# Field Findings

This note records the highest-signal findings from the current pre-`1.0`
field-validation phase.

It is intentionally short.

It is not a replacement for:

- [docs/field-validation.md](/Users/Shared/chroot/dev/gewyvern/docs/field-validation.md)
- [docs/1.0-readiness.md](/Users/Shared/chroot/dev/gewyvern/docs/1.0-readiness.md)

Instead, it answers a narrower question:

- what has already been observed in real validation work
- what currently looks stable
- what still looks conservative rather than strongly diagnostic

## Current Stable Findings

### 1. Registry Validation Is Fully Green

The built-in protocol registry currently contains `90` package entries, and the
entire scanned registry now passes:

- machine-facing `gewyc envelope --json` validation
- `gewyvern --scan-all --json --summary-only`

This means the current stable protocol shelf is no longer drifting at the
compiler/package level.

### 2. Packaged Linux Protocol Support Works After Real Install

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

### 3. Packaged Standalone Runtime Works In Clean Linux Containers

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

### 4. High-Value Operator Paths Already Look Conservative And Coherent

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

### 5. A Release-Style Packaged Linux Checklist Already Passes

The Debian release-style container validation path now passes as one deliberate
checklist run:

- package install smoke
- packaged runtime validation
- packaged protocol validation
- packaged operator-path validation

This matters because it is stronger than saying “individual scripts look good”.

It means the packaged Linux path can already be exercised as one release-minded
routine, not only as disconnected local checks.

### 6. The Three-Module Stack Already Works In Containers

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

The current pre-`1.0` line now looks strong in these ways:

- protocol/package shelf is stable
- packaged standalone runtime works
- packaged high-frequency protocol families work
- packaged operator paths stay conservative and coherent
- packaged Linux release-style validation can run as one checklist
- the current three-module collaboration topology already works in Docker

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
