# Field Validation

This note defines the practical field-validation phase for the active
`2.5.x` line.

The goal is simple:

- stop judging readiness only from architecture and contract cleanup
- start judging readiness from repeated real-world operator use
- define which outputs must stay stable enough under that use

## Post-Split Handoff

The [Leselang product cleanup boundary](leselang-repository-handoff.md#product-cleanup-closeout)
is closed. Retained legacy API facades, product adapters and the independent
language's future full-VM work do not block Gewyvern capability field testing.
The runtime dependency gate keeps Gewyvern itself independent of the control
stack; its development-only integration tests are a separate concern.

Resume the existing native validation entrypoints rather than building a new
parallel harness. The next campaign should cover these ordered layers:

1. Establish the server/kernel/toolchain and packaged-binary identity, then verify standalone CLI, service startup, stop and restart behavior.
2. Cross-check high-frequency and mixed protocol evidence through `registry`, `high-frequency` and `debugger-cross`, including negative cases that must not overclaim.
3. Exercise `runtime-operator` and `runtime-lifecycle` for malformed input, recovery and continued service readability. `resilience-roundtrip` only prepares helpers/configuration/runbook files; do not count it as executed fault injection.
4. On the authorized Linux lab, run real attach/kprobe/TC checks and the owned target-container scenario. Use a dedicated TC test interface and leave unrelated services, containers and traffic-control rules intact.
5. Run `linux-tcp-capture` on the host to correlate owned loopback socket cookies with real kernel TCP states, reconstruct sessions and replay exports. Add `--juice-shop` for owned private-container HTTP 403/500 request-to-flow correlation and an unselected HTTP 200 negative control. Use the mutually exclusive `--http-boundaries` mode for a real split response and misleading status text in a normal response body. Keep client I/O, raw BPF facts, packet-local DSL candidates and SYN-ACK-anchored derived prefixes separate. These bounded fixtures do not prove general stream reassembly, vulnerability detection or arbitrary process capture.
6. Run the mutually exclusive `linux-tcp-capture --capture-recovery` mode for actual map-write loss, packet/TCP event limits, an unreported snapshot gap, and owned child error/abort/SIGKILL recovery. Keep rejected captures invalid and verify both link detachment and identity-scoped cleanup before fresh capture. This is bounded fault injection, not sustained-load acceptance or supervisor/host crash recovery.
7. Use `linux-tcp-capture --capture-load` for a bounded baseline/capture/capture/baseline pressure comparison: four sixty-second phases, cgroup CPU and memory, application RTT, explicit capacity rejection, and a fresh post-pressure capture. The 128-slot snapshot control saturates during warmup; this measures the discard path, not continuous capture delivery.
8. Use `linux-tcp-capture --capture-stream` for the bounded 1-MiB ring-to-Rust path: baseline, continuous consumption, paused-consumer overflow/resume, repeated consumption and final baseline. Preserve counter reconciliation, per-phase completeness and actual kernel-to-sink latency. A successful fault campaign does not turn the intentionally lossy phase into complete evidence. Full runtime-ledger ingest, durable storage, arbitrary-network loss and open-loop tails remain separate acceptance work.
9. Use `linux-tcp-capture --capture-runtime` to add actual runtime ingestion and analysis in independent, bounded 256-fact sessions. Reconcile analyzed facts with deliveries, verify representative JSON samples through fresh-session reanalysis, and preserve explicit incompleteness under a slow consumer. Batch completion latency is distinct from sink acceptance; cross-batch session state, full-stream exports and durability are not covered.
10. Use `linux-tcp-capture --capture-mixed` for variable payloads, split/coalesced writes, half-close completion, fresh socket identities and a same-endpoint unselected neighbor. Check contiguous TCP byte ranges and actual prefixes independently of application message counts. Reject gaps/retransmission/overlap rather than claiming generalized reassembly; keep full-byte application checks separate from prefix-only capture. This is a finite correctness campaign, not a throughput or vulnerability-detection claim.
11. Use `linux-tcp-capture --auth-breadth` to connect real FTP control and LDAP simple-bind targets to the same kernel evidence path. For each protocol, compare selected success/denial with an unselected denied neighbor, reconstruct only complete bounded server bytes, and run the existing protocol packages with JSON replay. Unsupported process/route probes remain explicitly unavailable. This does not certify the whole protocol catalog, TLS decryption, request-side payloads, FTP data transfer or LDAP search/modify/SASL.

12. Use `linux-udp-capture` for private CoreDNS queries with two owned IPv4 UDP sockets: success, FORMERR/SERVFAIL/NXDOMAIN/REFUSED, a truncated reply, a dropped reply with bounded timeout and subsequent recovery. Reconcile both captured directions byte-for-byte against native client I/O, exclude a same-target neighbor, and replay packet-only DNS packages. Shared DNS transaction IDs must not merge socket identities. TC is not successful resolution, timeout is not a DNS error code, and transport phases are not full DNS parsing.
13. Use `linux-tcp-capture --data-breadth` for private PostgreSQL/Redis targets: authentication refusal, successful queries/reads, query/command errors, same-connection recovery, missing keys and error-looking normal values. Schema 2 adds explicit transaction failure/rollback, NULL versus empty text, binary Redis values and pipelined replies. Compare complete SYN-ACK-anchored server streams with bounded native client I/O and replay six existing packages, excluding same-target neighbors. Seven-byte client reads are not a claim of forced TCP segmentation. The finite 1024-byte prefix does not change the 64-byte streaming ABI. PostgreSQL error candidates use a conservative English field layout, not a full decoder; authentication-message/ReadyForQuery/bulk stages alone do not prove login/query/key success.

14. Use `linux-tcp-capture --mysql-memcached` for private MySQL 8 native-password and Memcached binary targets. Check MySQL login refusal versus query syntax errors, NULL versus empty rows and same-connection recovery. Check Memcached missing keys, APPEND `NOT_STORED`, oversized SET rejection, unknown commands, empty/binary values and successful reads after errors. Validate every matched packet's status, not only aggregate socket membership; replay seven existing packages and exclude same-target neighbors. The runner requires preloaded digest-pinned images and removes its own targets and networks.

15. Use `linux-tcp-capture --messaging-breadth` for real Mosquitto/NATS targets. MQTT checks identifier refusal, a seeded persistent-session resume, QoS 1/2 acknowledgements with exact IDs, resumed subscription delivery and ping. NATS checks authentication denial, permission-error recovery on the same connection, and empty/binary/control-looking message bodies. Reconcile complete server bytes, replay nine packages and exclude same-target neighbors. The seed is setup, not captured evidence; in-memory session resume is not crash durability. Message acknowledgements do not prove downstream persistence.
16. Use `linux-tcp-capture --smtp-breadth` for a separate real Mailpit mail-submission target. Check multiline replies, auth and recipient refusal, command-order errors and two accepted DATA transactions after recovery. Reconcile complete server bytes and all emitted receive-stage candidates; NOOP/RSET and recipient errors must not become message outcomes. Eight packages are replayed, with negative-only real coverage for post-body denial. No TLS, mailbox content, external delivery or crash-durability claim.
17. Use `linux-tcp-capture --pop3-breadth` for a fresh real Mailpit mailbox retrieval target. Seed through uncaptured native SMTP, then check password/unauthorized errors, LIST/UIDL/RETR, dot-stuffing and DELE/RSET recovery. Reconcile all raw bytes and replay three packages without classifying bare `+OK` or generic `-ERR`. Report the pinned server's extra RETR CRLF explicitly, not as conformant octet counts. No TLS, APOP/SASL, general dialogue correlation or deletion-durability claim.
18. Use `linux-tcp-capture --rtsp-breadth` for isolated MediaMTX RTSP/1.0 control. A native publisher supplies private SDP before capture. Verify missing-path 404/EOF then a fresh successful connection, UDP SETUP refusal followed by TCP SETUP, PLAY/keepalive/PAUSE/resume/TEARDOWN, exact CSeq/session identities and server-byte reconciliation. Replay four packages and packet-check every emitted receive stage. Complete headers are bounded to 512 bytes in product predicates; this is not TLS/authentication, RTSP/2.0, general stream reconstruction or RTP quality evidence.

### Breadth Acceptance

Count a capability by its evidence chain, not by the number of package files:
real target response -> owned kernel identity/bytes -> runtime facts -> existing
package stages -> replay -> negative controls -> cleanup and fresh capture.
Keep packet-local candidates separate from framed, origin-checked conclusions.
Client responses and target logs are independent oracles, never replacement facts.

The HTTP and FTP/LDAP fixtures cover different response framing on IPv4 TCP;
the UDP/DNS fixture adds bounded bidirectional datagram correctness, and the
PostgreSQL/Redis and MySQL/Memcached fixtures add real database/cache errors and
subsequent recovery. Mosquitto/NATS add bounded broker session and message exchange
coverage, not whole messaging-system acceptance.
Next breadth work should target IPv6, broader UDP, encrypted-transport
observability and additional database/cache protocols or advanced exchanges.
General TCP reassembly, localized/reordered PostgreSQL errors, TLS/SCRAM,
extended queries, RESP3/cluster/persistence, MySQL caching-SHA2/auth switching/
prepared statements, Memcached text/meta/SASL and database/cache performance remain
separate gates: neither the hour-scale load proof nor the protocol registry
count closes them. A failed or unsupported row stays visible rather than
becoming a skip-pass.

For runtime batch profiling, compare wall time with `CLOCK_THREAD_CPUTIME_ID`.
Batch stages sample every call; conversion and ingestion sample every 257th
call, avoiding a fixed position in the 256-fact batch. Only paired samples are
compared: sampled CPU totals must not be treated as whole-stage totals. Timers
include their own overhead, which dominates very short operations. The ignored
`profile_runtime_sink_stages` fixture includes an empty-timer calibration and
test-only thread-local allocation counts (requested bytes, not RSS).
It separately measures sink teardown, including the final retained buffers;
that one-time cleanup is outside the fixed-input timed region.

Use identical fixed input under separate CPU quotas before attributing long
release times to allocation work. Wall-minus-thread-CPU is a non-CPU residual,
not a direct quota-wait counter; it can include preemption, blocking and timer
overhead. Cgroup `cpu.stat` covers the whole group, not one consumer or stage.
Preserve both the CPU/wall pairs and cgroup counter deltas, then confirm any
optimization in real capture without the test allocation counter. Do not run
builds or regression tests concurrently with live performance measurements.
The measurement semantics follow the Linux [thread CPU clock documentation](https://man7.org/linux/man-pages/man3/clock_gettime.3.html)
and [cgroup CPU controller documentation](https://www.kernel.org/doc/html/latest/admin-guide/cgroup-v2.html).

See the [native command map](script-entrypoints.md) for command arguments and
evidence paths. Record source revision, target identity, kernel, configuration
and per-stage `passed`/`failed`/`skipped` outcomes. Missing privileges or absent
hardware are explicit gaps, never a pass. The cleanup's local compile and
contract results are not evidence that these Linux stages have already run.

## Validation Context

This is the bridge between:

- contract freeze
- benchmark acceptance
- current release judgment for the active `2.5.x` line

For the sealed `1.0.0` release posture and the final pre-stable checkpoint,
see [1.0.0 release posture](history/v1.0.0.md) and
[the pre-stable checkpoint](history/v0.20.x.md).

For the short running record of what this validation has already shown in
practice, see [field findings](field-findings.md).

For the shorter goal-based script map that now sits above the individual
validation shelves, see
[native validation commands](script-entrypoints.md).

For the concrete release gate, packaged artifact checklist, and ship/no-ship
decision shelf, use
[release checklist](release-checklist.md).

## Role In The Shelf

Treat this page as the validation-program page for the active line.

Use it when the question is:

- what kinds of validation should we still be running repeatedly?
- which scenario families matter most for trust in the current release?
- what counts as strong enough validation evidence versus shallow smoke?

Do not use this page as:

- the shortest ship/no-ship checklist
- the current release claim in one paragraph
- the running record of what already passed

For those, use:

- [Release checklist](release-checklist.md)
- [1.0.0 release posture](history/v1.0.0.md)
- [Pre-stable checkpoint](history/v0.20.x.md)
- [Field findings](field-findings.md)

## Companion Shelves

- [Field findings](field-findings.md)
  for the short evidence log of what this validation line has already shown
- [Release checklist](release-checklist.md)
  for the shortest current release gate
- [1.0.0 release posture](history/v1.0.0.md)
  for the stable line's intended claim and scope
- [Pre-stable checkpoint](history/v0.20.x.md)
  for the final pre-stable sealing context
- [Protocol and host-validation baseline](history/v0.18.x.md)
  for the protocol breadth and physical-host validation baseline this line
  inherits

## Current Prelaunch Scope

Keep the current `2.5.x` field-validation line intentionally narrow.

The question is not "what else could `gewyvern` support".

The question is:

- what most improves trust in the current stable release line
- what should stay frozen until the next deliberate contract change

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
3. Keep debugger cross-validation as release evidence:
   - runtime summary JSON
   - debugger-console JSON
   - `gewyc` envelope JSON
   - negative cases that must not overclaim
4. Tune built-in operator guidance only in small, standalone-useful ways:
   - `observe_more`
   - `manual_review`
   - `targeted_ready`
5. Accept only small IR improvements that reduce boilerplate or improve
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

This section is about validation priority, not product marketing and not the
final ship decision. A line can have a clear posture and still need more field
validation repetitions before the release gate should be called green.

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
- `cargo run -- --dsl dsl/http_request_path.gewy --json --summary-only`
- `cargo run -p gewyc -- explain dsl/http_request_path.gewy --json`
- [scripts/validation/high_frequency_validation.sh](../scripts/validation/high_frequency_validation.sh)

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
- [scripts/validation/registry_validation.sh](../scripts/validation/registry_validation.sh)

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

- `cargo run --quiet --bin gewyvern_validate -- socket-roundtrip`
  Socket session roundtrip through the standalone ingest path. The legacy
  [scripts/demos/socket_roundtrip_demo.sh](../scripts/demos/socket_roundtrip_demo.sh)
  wrapper remains for older automation.
- [scripts/validation/runtime_operator_validation.sh](../scripts/validation/runtime_operator_validation.sh)
  Broader serve/API/runtime operator validation path.
- local `--serve` plus read-only API usage

Pair this validation track with the operator-facing preflight at
[Security checklist](book/how-to-security-checklist.md)
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

- `cargo run --quiet --bin gewyvern_validate -- field-smoke`
- `cargo run --quiet --bin gewyvern_validate -- high-frequency`
- `cargo run --quiet --bin gewyvern_validate -- release-container-check`
- `cargo run --quiet --bin gewyvern_validate -- container-validation-summary`
- `cargo run --quiet --bin gewyvern_validate -- container-protocol-validation`
- `cargo run --quiet --bin gewyvern_validate -- container-operator-path-validation`
- `cargo run --quiet --bin gewyvern_validate -- registry`
- `cargo run --quiet --bin gewyvern_validate -- runtime-operator`
- `cargo run --quiet --bin gewyvern_validate -- container-runtime-validation`
- [scripts/validation/field_validation_smoke.sh](../scripts/validation/field_validation_smoke.sh)
- [scripts/validation/high_frequency_validation.sh](../scripts/validation/high_frequency_validation.sh)
- [scripts/packaging/release_container_check.sh](../scripts/packaging/release_container_check.sh)
- [scripts/packaging/container_validation_summary.sh](../scripts/packaging/container_validation_summary.sh)
- [scripts/packaging/container_protocol_validation.sh](../scripts/packaging/container_protocol_validation.sh)
- [scripts/packaging/container_operator_path_validation.sh](../scripts/packaging/container_operator_path_validation.sh)
- [scripts/validation/registry_validation.sh](../scripts/validation/registry_validation.sh)
- [scripts/validation/runtime_operator_validation.sh](../scripts/validation/runtime_operator_validation.sh)
- [scripts/packaging/container_runtime_validation.sh](../scripts/packaging/container_runtime_validation.sh)
- [Release checklist](release-checklist.md)
  Release-facing packaged and multi-project gate.

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

The exact packaged release gate, rebuild order, and three-module collaboration
gate are intentionally described in
[Release checklist](release-checklist.md)
rather than duplicated here. This page treats them as one validation track
inside the wider field-validation program.

The container protocol validator is the shortest repeatable check that packaged
protocol support still works after native install, including grouped
high-frequency protocol families and packaged `--scan-all` outside the
development host.

The registry validator is the shortest repeatable check that scanned protocol
packages still compile through the current path, machine-facing compiler JSON
has not silently drifted, and failures are isolated to concrete packages
instead of only surfacing as a broad `--scan-all` break.

The runtime/operator validator is the shortest repeatable check that live
`--serve` plus the read-only API work on a clean local bind, repeated
socket-fed sessions refresh the latest snapshot, and operator-facing
`summary.json`, `export.json`, and `analysis.json` stay readable through a real
service workflow.

The container runtime validator is the shortest repeatable check that the
packaged Linux runtime still works after a real install step, `--serve`,
socket ingest, and the read-only API stay coherent in clean Linux
environments, and malformed ingest does not kill the packaged service loop.

The container operator-path validator is the shortest repeatable check that
packaged Linux installs preserve high-value operator-path protocol chains and
that denied or setup-incomplete demos do not over-collapse into stronger
failure claims than the current evidence supports.

## What This Phase Still Does Not Replace

This note does not replace:

- full regression tests
- benchmark acceptance
- real operator feedback
- release gate judgment
- the dedicated multi-project collaboration gate with `etragon` and `leserpent`

It exists so the active `2.5.x` line has a concrete "keep validating for
real" shelf rather
than only architecture cleanup.
