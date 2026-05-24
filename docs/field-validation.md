# Field Validation

This note defines the practical field-validation phase for `gewyvern`
`v0.10.0`.

The goal is simple:

- stop judging readiness only from architecture and contract cleanup
- start judging readiness from repeated real-world operator use
- define which outputs must stay stable enough under that use

This is the bridge between:

- contract freeze
- benchmark acceptance
- eventual `v1.0.0` release judgment

For the narrower release checklist, see
[docs/1.0-readiness.md](/Users/Shared/chroot/dev/gewyvern/docs/1.0-readiness.md).

For the short running record of what this validation has already shown in
practice, see [docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md).

## What Counts As Validation

Validation here means:

- real CLI invocations
- long-lived `--serve` behavior
- socket ingest behavior
- repeated reading of `summary.json`, `analysis.json`, and target routing
- realistic protocol mixes and failure paths
- packaged Linux protocol behavior after real install

It does not mean only unit tests or synthetic compiler checks.

## Stability Anchors

For field validation, the main outputs we care about are:

- `primary_failure_*`
- `operator_guidance_*`
- `ambiguous`
- `competing_hypotheses`
- `pid_attribution_status`
- `process_network_profiles`
- `protocol_flows`

The question is not "does every byte stay identical".

The question is:

- does the diagnosis spine stay conservative
- does the next-step guidance stay coherent
- do the same classes of scenarios stay within an expected output range

## Validation Tracks

### 1. Standalone CLI Smoke

Purpose:

- confirm that a fresh local checkout still behaves like a usable standalone
  debugger

Current commands to keep exercising:

- `cargo run -- --demo udp --json --summary-only`
- `cargo run -- --dsl /Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy --json --summary-only`
- `cargo run -p gewyc -- explain /Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy --json`
- [scripts/high_frequency_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/high_frequency_validation.sh)

Expected qualities:

- commands complete without operator guesswork
- JSON output contains the stable diagnosis spine
- compiler/debugging surfaces still expose `summary` and `next_step`

Registry-wide package validation should still be exercised, but it is a
separate track from the minimal standalone smoke path because it verifies the
scanned gewy package shelf as a whole rather than only the core runtime shell.

### 1b. Registry And Sweep Validation

Purpose:

- confirm that built-in scanned protocol packages remain compatible with the
  current stable compiler/runtime path

Current commands to keep exercising:

- `cargo run -- --list-protocols`
- `cargo run -- --scan-all --json --summary-only`
- [scripts/registry_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/registry_validation.sh)

Expected qualities:

- registry scan completes without compiler/runtime breakage
- target enumeration and summary output remain operator-usable
- protocol package drift is discovered explicitly rather than being silently
  masked by the core runtime smoke path

The registry validator is intentionally more granular than `--scan-all`.

It checks each scanned package individually and distinguishes:

- command failure
- JSON-shape failure
- parse-stage failure
- validation-stage failure
- diagnostics-stage failure

That makes it a better first stop when the registry shelf drifts.

### 2. Socket Session Validation

Purpose:

- confirm that the standalone service shape still works without external
  orchestration

Current commands to keep exercising:

- [scripts/socket_roundtrip_demo.sh](/Users/Shared/chroot/dev/gewyvern/scripts/socket_roundtrip_demo.sh)
- [scripts/runtime_operator_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/runtime_operator_validation.sh)
- local `--serve` plus read-only API usage

Expected qualities:

- bounded socket ingest still accepts healthy local sessions
- latest snapshot output is refreshed correctly
- malformed sessions do not kill the `--serve` loop
- operator-facing JSON remains readable after a real session path

This track may require local socket-bind permissions that are not available in
every sandboxed development environment. It should still be exercised
explicitly during field validation, but it is not part of the minimal default
smoke path.

### 3. Failure-Mode Validation

Purpose:

- confirm that important negative paths still land in stable diagnosis ranges

Core scenario families:

- direct protocol denial
- missing follow-up / no response
- setup incomplete
- semantic error
- ambiguous mixed-path cases

Expected stable range:

- `primary_failure_mode` stays within the expected family
- `primary_failure_confidence` stays within the expected confidence band
- `operator_guidance_action` stays aligned with the failure class
- `ambiguous` and `competing_hypotheses` appear when the runtime is refusing to
  over-collapse

### 4. Mixed-Flow Validation

Purpose:

- confirm that realistic mixed evidence does not cause overconfident collapse

Core scenario families:

- `DNS + TLS + HTTP`
- `proxy auth + upstream request`
- `QUIC + HTTP/3`

Expected stable range:

- the runtime may choose a different primary module candidate over time only if
  the result remains conservative
- `ambiguous=true` and low-confidence summaries are acceptable here
- stronger direct-signal collapse is only acceptable when the evidence is
  actually stronger

### 5. Long-Lived Service Validation

Purpose:

- confirm that `--serve` remains useful as a standalone latest-snapshot
  analysis service

Core things to observe:

- later sessions replace earlier latest snapshots cleanly
- bad sessions do not kill the serve loop
- read-only API behavior remains coherent
- degraded/external-engine failure behavior stays additive, not catastrophic
- packaged Linux installs can pass the same standalone runtime checks inside
  clean containers

## Initial Acceptance Rules

For this phase, treat the following as pass conditions:

1. the standalone smoke path completes locally
2. the socket roundtrip path still produces a usable JSON export
3. failure-path outputs remain inside an expected conservative range
4. mixed-flow outputs do not silently become overconfident
5. long-lived service behavior stays additive and bounded

## Initial Smoke Entry Point

The current local smoke entry point is:

- [scripts/field_validation_smoke.sh](/Users/Shared/chroot/dev/gewyvern/scripts/field_validation_smoke.sh)
- [scripts/high_frequency_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/high_frequency_validation.sh)
- [scripts/container_protocol_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/container_protocol_validation.sh)
- [scripts/container_operator_path_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/container_operator_path_validation.sh)
- [scripts/registry_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/registry_validation.sh)
- [scripts/runtime_operator_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/runtime_operator_validation.sh)
- [scripts/container_runtime_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/container_runtime_validation.sh)

It is intentionally small.

It is not the whole field-validation program.

It is the shortest repeatable check that:

- the standalone CLI still works
- the compiler/debugger surface still works
- socket roundtrip still works

The high-frequency validator is the shortest repeatable check that:

- common operator paths still map to the expected primary module families
- mixed-flow regression tests still preserve conservative collapse
- built-in operator guidance remains coherent on high-frequency standalone paths

The container protocol validator is the shortest repeatable check that:

- packaged protocol support still works after native install
- high-frequency protocol families such as DNS, HTTP, TLS, HTTP/3, QUIC, SSH,
  SOCKS5, MySQL, PostgreSQL, SMTP, and LDAP keep their expected
  module/guidance shape in clean Linux environments
- packaged `--scan-all` still works outside the development host

The registry validator is the shortest repeatable check that:

- scanned protocol packages still compile through the current path
- machine-facing compiler JSON has not silently drifted
- failures are isolated to concrete packages instead of only surfacing as a
  broad `--scan-all` break

The runtime/operator validator is the shortest repeatable check that:

- live `--serve` plus read-only API works on a clean local bind
- repeated socket-fed sessions refresh the latest snapshot
- operator-facing `summary.json`, `export.json`, and `analysis.json` stay
  readable through a real service workflow

The container runtime validator is the shortest repeatable check that:

- the packaged Linux runtime still works after a real install step
- `--serve`, socket ingest, and the read-only API stay coherent in clean
  Linux environments
- malformed ingest does not kill the packaged service loop

The container operator-path validator is the shortest repeatable check that:

- packaged Linux installs preserve high-value operator-path protocol chains
- `DNS -> QUIC -> HTTP/3` keeps a conservative handoff from name resolution to
  transport setup to application response
- `DNS -> TLS -> HTTPS CONNECT` keeps a conservative secure-transport /
  tunnel-establishment posture in clean Linux environments
- `DNS -> SOCKS5 -> HTTPS CONNECT` keeps a conservative proxy-auth /
  tunnel-establishment posture in clean Linux environments
- `DNS -> TLS -> Postgres` keeps a conservative secure-database /
  query-establishment posture in clean Linux environments
- `DNS -> SMTP` keeps its expected setup-incomplete / observe-more posture
- current packaged denied demos such as `SOCKS5 auth denied` must not
  over-collapse into stronger failure claims when the synthetic evidence is
  still only a setup-incomplete path

## What This Phase Still Does Not Replace

This note does not replace:

- full regression tests
- benchmark acceptance
- real operator feedback
- later multi-project collaboration validation with `etragon` and `leserpent`

It exists so `v0.10.0` has a concrete "start validating for real" shelf rather
than only architecture cleanup.
