# Field Validation

This note defines the practical field-validation phase for the current
`0.14.x` line.

The goal is simple:

- stop judging readiness only from architecture and contract cleanup
- start judging readiness from repeated real-world operator use
- define which outputs must stay stable enough under that use

This is the bridge between:

- contract freeze
- benchmark acceptance
- current release judgment for the active `0.14.x` line

For the current release posture, see
[docs/v0.14-posture.md](/Users/Shared/chroot/dev/gewyvern/docs/v0.14-posture.md).

For the short running record of what this validation has already shown in
practice, see [docs/field-findings.md](/Users/Shared/chroot/dev/gewyvern/docs/field-findings.md).

## Current Prelaunch Scope

Keep the current `0.14.x` field-validation line intentionally narrow.

The question is not "what else could `gewyvern` support".

The question is:

- what most improves trust in the current stable release line
- what should stay frozen until the next deliberate line change

### Do Next

1. Deepen high-frequency protocol stability:
   - `HTTP / HTTPS / TLS`
   - `DNS`
   - `SSH`
   - `SOCKS5 / proxy`
   - `MySQL / PostgreSQL`
   - `QUIC / HTTP/3`
2. Strengthen mixed-flow conservatism:
   - `DNS + TLS + HTTP`
   - `proxy auth + upstream request`
   - `QUIC + HTTP/3`
3. Tune built-in operator guidance only in small, standalone-useful ways:
   - `observe_more`
   - `manual_review`
   - `targeted_ready`
4. Accept only small IR improvements that reduce boilerplate or improve
   lowering/diagnostic stability.

### Do Not Expand Right Now

Keep the following out of the prelaunch scope:

- adding whole new protocol families for coverage alone
- introducing new major IR layers
- renaming the core diagnosis spine
- widening the DSL in ways that would reactivate registry churn

### Working Rule

Before taking a protocol or IR change, ask:

1. does this improve trust in a common operator workflow?
2. does this reduce ambiguity or drift in an already-supported high-frequency path?
3. can this be done without reactivating broad surface churn?

If the answer is "no", it probably belongs after the prelaunch line rather than
before it.

## What Counts As Validation

Validation here means:

- real CLI invocations
- long-lived `--serve` behavior
- socket ingest behavior
- repeated reading of `summary.json`, `analysis.json`, and target routing
- realistic protocol mixes and failure paths
- packaged Linux protocol behavior after real install

The packaged protocol path is no longer just the first high-frequency shelf.
It now also exercises a second grouped family covering:

- cache access and brokers:
  `Redis`, `MQTT`, `AMQP`
- auth and identity exchanges:
  `RADIUS`, `FTP`, `IMAP`, `POP3`, `Kerberos`
- management and signaling:
  `SNMP`, `RTSP`

It does not mean only unit tests or synthetic compiler checks.

Throughout this note, the script naming split is intentional:

- `smoke` means a lightweight bring-up or existence check
- `roundtrip` means one narrow end-to-end consumer path
- `validation` means a grouped expectation check for one surface family
- `summary` means a wrapper that runs several validations together

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
  Socket session roundtrip through the standalone ingest path.
- [scripts/runtime_operator_validation.sh](/Users/Shared/chroot/dev/gewyvern/scripts/runtime_operator_validation.sh)
  Broader serve/API/runtime operator validation path.
- local `--serve` plus read-only API usage

Pair this validation track with the operator-facing preflight at
[docs/book/how-to-security-checklist.md](/Users/Shared/chroot/dev/gewyvern/docs/book/how-to-security-checklist.md)
so deployment intent and runtime behavior are checked together.

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
- [scripts/release_container_check.sh](/Users/Shared/chroot/dev/gewyvern/scripts/release_container_check.sh)
- [scripts/container_validation_summary.sh](/Users/Shared/chroot/dev/gewyvern/scripts/container_validation_summary.sh)
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

The container validation summary is the shortest repeatable check that:

- packaged Linux container validation can be run from one entrypoint
- packaged protocol validation and packaged operator-path validation stay in
  sync as a single routine

The release container check is the shortest repeatable check that:

- packaged install, runtime, protocol, and operator-path validation can all be
  exercised from one release-minded entrypoint
- the prelaunch Linux packaging path can be treated like a single checklist
  step rather than a loose collection of commands
- the current line is being validated against freshly rebuilt native artifacts,
  not only whatever package output happened to already exist under
  `target/packages`

The container protocol validator is the shortest repeatable check that:

- packaged protocol support still works after native install
- grouped high-frequency protocol families keep their expected module/guidance
  shape in clean Linux environments:
  - resolution, web, and secure transport:
    DNS, HTTP, TLS, HTTP/3, QUIC
  - remote access and proxy:
    SSH, SOCKS5
  - current HTTP/3 request posture remains explicitly high-signal:
    `primary_module_kind = "http3_request_response"` with
    `operator_guidance_action = "safe_to_escalate_protocol_signal"`
  - database, messaging, and directory:
    MySQL, PostgreSQL, SMTP, LDAP
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
- advisory resolution/application paths such as `DNS -> QUIC -> HTTP/3` keep a
  conservative handoff from name resolution to transport setup to application
  response
- secure transport/tunnel paths such as `DNS -> TLS -> HTTPS CONNECT` and
  `DNS -> SOCKS5 -> HTTPS CONNECT` keep a conservative tunnel-establishment
  posture in clean Linux environments
- secure database/mail paths such as `DNS -> TLS -> Postgres`,
  `DNS -> TLS -> MySQL`, `DNS -> TLS -> SMTP auth`, and `DNS -> SMTP` keep
  their current observe-more / setup-incomplete posture in clean Linux
  environments
- current packaged denied demos such as `SOCKS5 auth denied` must not
  over-collapse into stronger failure claims when the synthetic evidence is
  still only a setup-incomplete path

## What This Phase Still Does Not Replace

This note does not replace:

- full regression tests
- benchmark acceptance
- real operator feedback
- later multi-project collaboration validation with `etragon` and `leserpent`

It exists so the current `0.14.x` line has a concrete "start validating for
real" shelf rather
than only architecture cleanup.
